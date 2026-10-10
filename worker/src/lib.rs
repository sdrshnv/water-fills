use serde::Deserialize;
use shared::logic::{check_auth, state_from_aggregates, validate_fill};
use shared::{sql, FillRequest, State};
use worker::wasm_bindgen::JsValue;
use worker::*;

#[derive(Deserialize)]
struct StateRow {
    total: u32,
    pays: u32,
    last_kind: Option<String>,
    last_oz: Option<u32>,
}

async fn read_state(db: &D1Database) -> Result<State> {
    let row = db
        .prepare(sql::SELECT_STATE)
        .first::<StateRow>(None)
        .await?
        .ok_or_else(|| Error::RustError("state query returned no row".into()))?;
    let last = sql::last_from_columns(row.last_kind.as_deref(), row.last_oz);
    Ok(state_from_aggregates(row.total, row.pays, last))
}

/// Runs one write statement and returns how many rows it changed.
async fn changes(stmt: D1PreparedStatement) -> Result<usize> {
    let result = stmt.run().await?;
    Ok(result.meta()?.and_then(|m| m.changes).unwrap_or(0))
}

async fn state_response(db: &D1Database) -> Result<Response> {
    Response::from_json(&read_state(db).await?)
}

#[event(fetch)]
async fn fetch(mut req: Request, env: Env, _ctx: Context) -> Result<Response> {
    let url = req.url()?;
    let path = url.path();
    if !path.starts_with("/api/") {
        // Static assets are served by the platform; anything else is a 404.
        return Response::error("Not found", 404);
    }

    let key = env.secret("API_KEY")?.to_string();
    let header = req.headers().get("Authorization")?;
    if !check_auth(header.as_deref(), &key) {
        return Response::error("Unauthorized", 401);
    }

    let db = env.d1("DB")?;
    match (req.method(), path) {
        (Method::Get, "/api/state") => state_response(&db).await,
        (Method::Post, "/api/fill") => {
            let Ok(body) = req.json::<FillRequest>().await else {
                return Response::error("Bad request", 400);
            };
            let Ok(oz) = validate_fill(body.oz) else {
                return Response::error("Invalid fill amount", 400);
            };
            let stmt = db
                .prepare(sql::INSERT_FILL)
                .bind(&[JsValue::from_f64(oz as f64)])?;
            changes(stmt).await?;
            state_response(&db).await
        }
        (Method::Post, "/api/pay") => {
            if changes(db.prepare(sql::INSERT_PAY)).await? == 0 {
                return Response::error("Nothing owed", 409);
            }
            state_response(&db).await
        }
        (Method::Post, "/api/undo") => {
            if changes(db.prepare(sql::DELETE_LAST)).await? == 0 {
                return Response::error("Nothing to undo", 409);
            }
            state_response(&db).await
        }
        (_, "/api/state" | "/api/fill" | "/api/pay" | "/api/undo") => {
            Response::error("Method not allowed", 405)
        }
        _ => Response::error("Not found", 404),
    }
}
