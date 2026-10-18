use crate::combat_manager::distance_squared;
use crate::unit_manager::{GoalNode, MoveOrder, UnitManager, UnitOwner};

pub struct AiManager {}

impl AiManager {
    pub fn new() -> Self {
        Self {}
    }

    pub fn update(&mut self, unit_manager: &mut UnitManager) {
        if !unit_manager.is_initialized || !unit_manager.turn_tick_this_frame {
            return;
        }
        for unit_i in 0..unit_manager.len() {
            if unit_manager.owner[unit_i] != UnitOwner::P1 || unit_manager.is_dead[unit_i] {
                continue;
            }
            if unit_manager.move_period[unit_i] == 0 || unit_manager.target[unit_i].is_some() {
                continue;
            }
            if !unit_manager.goal_queue[unit_i].is_empty() {
                continue;
            }
            let Some(next) = Self::next_waypoint(unit_manager, unit_i) else {
                continue;
            };
            unit_manager.pending_orders.push(MoveOrder {
                unit_i,
                target: next,
                is_queued: false,
            });
        }
    }

    fn next_waypoint(unit_manager: &UnitManager, unit_i: usize) -> Option<GoalNode> {
        let waypoints = &unit_manager.waypoints[unit_i];
        if waypoints.is_empty() {
            return None;
        }
        let here = GoalNode {
            level_pos: unit_manager.level_pos[unit_i],
            tile: unit_manager.tile_position[unit_i],
        };
        for k in 0..waypoints.len() {
            if waypoints[k] != here {
                continue;
            }
            let next = waypoints[(k + 1) % waypoints.len()];
            if next == here {
                return None;
            }
            return Some(next);
        }

        let mut nearest = waypoints[0];
        let mut nearest_distance = i32::MAX;
        for waypoint in waypoints {
            if waypoint.level_pos != here.level_pos {
                continue;
            }
            let distance = distance_squared(waypoint.tile, here.tile);
            if distance < nearest_distance {
                nearest = *waypoint;
                nearest_distance = distance;
            }
        }
        Some(nearest)
    }
}
