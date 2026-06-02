//! Pathfinding helpers shared by exploration and combat behavior.

use crate::entity::{EntityId, GridPos};
use crate::world::{first_step_on_line, plan_path};

use super::ctx::BehaviorCtx;

/// One step along a path from `from` toward `goal`, or `None` if blocked/unreachable.
#[must_use]
pub fn next_step_toward(
    ctx: &BehaviorCtx<'_>,
    actor: EntityId,
    from: GridPos,
    goal: GridPos,
) -> Option<GridPos> {
    let plan = plan_path(
        ctx.map,
        ctx.entities,
        from,
        goal,
        Some(actor),
        true,
        u32::MAX,
    )
    .ok()?;
    let waypoint = plan.path.get(1).copied()?;
    first_step_on_line(from, waypoint)
}
