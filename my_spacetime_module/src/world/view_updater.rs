use spacetimedb::{ReducerContext, Table, Identity};
use crate::tables::chunk_entities::{chunk_entities, ChunkEntity};

pub fn upsert_entity(
    ctx: &ReducerContext,
    entity_id: Identity,
    entity_type: &str,
    pos_x: f32,
    pos_y: f32,
    chunk_x: i32,
    chunk_y: i32,
    data: Option<String>,
) {
    if let Some(mut row) = ctx.db.chunk_entities().entity_id().find(entity_id) {
        row.pos_x      = pos_x;
        row.pos_y      = pos_y;
        row.chunk_x    = chunk_x;
        row.chunk_y    = chunk_y;
        row.data       = data.clone();
        ctx.db.chunk_entities().entity_id().update(row);
    } else {
        ctx.db.chunk_entities().insert(ChunkEntity {
            entity_id,
            entity_type: entity_type.to_string(),
            pos_x,
            pos_y,
            chunk_x,
            chunk_y,
            data,
        });
    }
}

pub fn delete_entity(ctx: &ReducerContext, entity_id: Identity) {
    ctx.db.chunk_entities().entity_id().delete(&entity_id);
}