use crate::audio_manager::AudioManager;
use crate::history_manager::HistoryManager;
use crate::level::interactable::{Interactable, Rubble};
use crate::level::level_manager::LevelManager;
use crate::sfx::{SfxEvent, GATHER_RESOURCE_SFX};
use crate::unit_design::{
    can_attack, target_class, target_class_index, technology_bit, weapon_stats, Weapon,
    MEN_PER_UNIT, PLAYER_AGGRO_RANGE, RESOURCE_COUNT,
};
use crate::unit_manager::{AttackEvent, GatherEvent, GoalNode, UnitManager, UnitOwner, UnitType};
use macroquad::prelude::IVec2;
use std::collections::HashMap;
use std::mem;

pub struct CombatManager {
    scratch_level_units: HashMap<IVec2, Vec<usize>>,
}

pub fn distance_squared(a: IVec2, b: IVec2) -> i32 {
    let delta = a - b;
    delta.x * delta.x + delta.y * delta.y
}

fn scaled_damage(damage: &[i32; 3], victim_type: UnitType, men: i32) -> u16 {
    let base = damage[target_class_index(target_class(victim_type))];
    let scaled = (base * men + MEN_PER_UNIT as i32 - 1) / MEN_PER_UNIT as i32;
    scaled.max(1) as u16
}

fn splash_damage(damage: u16, target_tile: IVec2, victim_tile: IVec2) -> u16 {
    let delta = (victim_tile - target_tile).abs();
    damage >> delta.x.max(delta.y)
}

fn picks_own_target(unit_manager: &UnitManager, unit_i: usize) -> bool {
    let unit_type = unit_manager.unit_type[unit_i];
    if unit_manager.owner[unit_i] != UnitOwner::P0 || unit_type.is_structure() {
        return true;
    }
    unit_type == UnitType::Soldier && unit_manager.goal_queue[unit_i].is_empty()
}

fn is_beyond_fixed_range(unit_manager: &UnitManager, unit_i: usize, target_i: usize) -> bool {
    if unit_manager.move_period[unit_i] != 0 {
        return false;
    }
    let range = weapon_stats(unit_manager.profile[unit_i].design.weapon).range;
    let distance = distance_squared(
        unit_manager.tile_position[unit_i],
        unit_manager.tile_position[target_i],
    );
    distance > range * range
}

fn aggro_on_hit(
    unit_manager: &mut UnitManager,
    history: &mut HistoryManager,
    victim_i: usize,
    attacker_i: usize,
) {
    if unit_manager.is_dead[victim_i]
        || unit_manager.target[victim_i].is_some()
        || unit_manager.owner[victim_i] == unit_manager.owner[attacker_i]
        || !can_attack(
            unit_manager.unit_type[victim_i],
            &unit_manager.profile[victim_i].design,
        )
        || !picks_own_target(unit_manager, victim_i)
        || is_beyond_fixed_range(unit_manager, victim_i, attacker_i)
    {
        return;
    }
    unit_manager.set_target(victim_i, Some(attacker_i), history);
}

fn collect_reward(
    unit_manager: &mut UnitManager,
    history: &mut HistoryManager,
    impact_sfx: &mut Vec<SfxEvent>,
    unit_i: usize,
) {
    let reward = unit_manager.profile[unit_i].reward;
    if !reward.resources.is_empty() {
        history.record_event_resources(&unit_manager.economy.resources);
        unit_manager.economy.resources.add(&reward.resources);
        let amounts = reward.resources.amounts();
        for resource_i in 0..RESOURCE_COUNT {
            if amounts[resource_i] > 0 {
                impact_sfx.push(GATHER_RESOURCE_SFX[resource_i]);
            }
        }
    }
    let mut technology = None;
    if let Some(reward_technology) = reward.technology {
        if !unit_manager.economy.is_unlocked(Some(reward_technology)) {
            history.record_event_technology(unit_manager.economy.technology);
            unit_manager.economy.technology |= technology_bit(reward_technology);
            impact_sfx.push(SfxEvent::GatherTechnology);
            technology = Some(reward_technology);
        }
    }
    if reward.resources.is_empty() && technology.is_none() {
        return;
    }
    unit_manager.pending_gathers.push(GatherEvent {
        level_pos: unit_manager.level_pos[unit_i],
        tile: unit_manager.tile_position[unit_i],
        resources: reward.resources,
        technology,
    });
}

fn push_men_lost(
    unit_manager: &UnitManager,
    impact_sfx: &mut Vec<SfxEvent>,
    unit_i: usize,
    men_before: u16,
) {
    if unit_manager.unit_type[unit_i].is_structure() {
        return;
    }
    let mut men_after = 0;
    if !unit_manager.is_dead[unit_i] {
        men_after = unit_manager.men(unit_i);
    }
    for _ in men_after..men_before {
        impact_sfx.push(SfxEvent::DestroyMan);
    }
}

fn leave_rubble(
    unit_manager: &UnitManager,
    level_manager: &mut LevelManager,
    history: &mut HistoryManager,
    unit_i: usize,
) {
    if !unit_manager.unit_type[unit_i].is_structure() {
        return;
    }
    let level_pos = unit_manager.level_pos[unit_i];
    let tile = unit_manager.tile_position[unit_i];
    let Some(level) = level_manager.get_level_at_pos_mut(level_pos) else {
        return;
    };
    if level.interactables.contains_key(&tile) {
        return;
    }
    history.record_event_interactable(level_pos, tile, None);
    level.interactables.insert(
        tile,
        Interactable::Rubble(Rubble {
            is_from_combat: true,
        }),
    );
}

fn kill(
    unit_manager: &mut UnitManager,
    level_manager: &mut LevelManager,
    history: &mut HistoryManager,
    audio_manager: &mut AudioManager,
    impact_sfx: &mut Vec<SfxEvent>,
    unit_i: usize,
) {
    if unit_manager.is_dead[unit_i] {
        return;
    }
    unit_manager.remove_unit(unit_i, history);
    leave_rubble(unit_manager, level_manager, history, unit_i);
    match unit_manager.unit_type[unit_i] {
        UnitType::Soldier => {}
        UnitType::Machine => impact_sfx.push(SfxEvent::DestroyMachine),
        UnitType::Structure => impact_sfx.push(SfxEvent::DestroyStructure),
    }
    if unit_manager.profile[unit_i].is_final_fortress {
        audio_manager.pending_sfx.push(SfxEvent::GameWin);
    }
    if unit_manager.owner[unit_i] != UnitOwner::P1 {
        if !unit_manager.unit_type[unit_i].is_structure() && unit_manager.is_game_lost() {
            audio_manager.pending_sfx.push(SfxEvent::GameLose);
        }
        return;
    }
    collect_reward(unit_manager, history, impact_sfx, unit_i);
}

fn deal_damage(
    unit_manager: &mut UnitManager,
    level_manager: &mut LevelManager,
    history: &mut HistoryManager,
    audio_manager: &mut AudioManager,
    impact_sfx: &mut Vec<SfxEvent>,
    attacker_i: usize,
    victim_i: usize,
    damage: u16,
) {
    if unit_manager.is_dead[victim_i] {
        return;
    }
    let men_before = unit_manager.men(victim_i);
    history.record_event_hp(victim_i as u32, unit_manager.hp[victim_i]);
    unit_manager.hp[victim_i] = unit_manager.hp[victim_i].saturating_sub(damage);
    if unit_manager.hp[victim_i] > 0 {
        push_men_lost(unit_manager, impact_sfx, victim_i, men_before);
        return;
    }
    kill(
        unit_manager,
        level_manager,
        history,
        audio_manager,
        impact_sfx,
        victim_i,
    );
    push_men_lost(unit_manager, impact_sfx, victim_i, men_before);
    if unit_manager.owner[victim_i] != unit_manager.owner[attacker_i] {
        history.record_event_kills(attacker_i as u32, unit_manager.kills[attacker_i]);
        unit_manager.kills[attacker_i] = unit_manager.kills[attacker_i].saturating_add(1);
    }
    if !unit_manager.profile[victim_i].is_final_fortress {
        return;
    }
    let level_pos = unit_manager.level_pos[victim_i];
    for unit_i in 0..unit_manager.len() {
        if unit_manager.is_dead[unit_i]
            || unit_manager.owner[unit_i] != UnitOwner::P1
            || unit_manager.level_pos[unit_i] != level_pos
        {
            continue;
        }
        let men_before = unit_manager.men(unit_i);
        kill(
            unit_manager,
            level_manager,
            history,
            audio_manager,
            impact_sfx,
            unit_i,
        );
        push_men_lost(unit_manager, impact_sfx, unit_i, men_before);
    }
}

impl CombatManager {
    pub fn new() -> Self {
        Self {
            scratch_level_units: HashMap::new(),
        }
    }

    pub fn update(
        &mut self,
        unit_manager: &mut UnitManager,
        level_manager: &mut LevelManager,
        history: &mut HistoryManager,
        audio_manager: &mut AudioManager,
    ) {
        if !unit_manager.is_initialized || !unit_manager.turn_tick_this_frame {
            return;
        }
        self.rebuild_level_units(unit_manager);
        self.drop_lost_targets(unit_manager, history);
        self.acquire_targets(unit_manager, history);
        self.attack(unit_manager, level_manager, history, audio_manager);
        self.chase(unit_manager, history);
    }

    fn rebuild_level_units(&mut self, unit_manager: &UnitManager) {
        for units in self.scratch_level_units.values_mut() {
            units.clear();
        }
        for unit_i in 0..unit_manager.len() {
            if unit_manager.is_dead[unit_i] {
                continue;
            }
            let level_pos = unit_manager.level_pos[unit_i];
            match self.scratch_level_units.get_mut(&level_pos) {
                Some(units) => units.push(unit_i),
                None => {
                    self.scratch_level_units.insert(level_pos, vec![unit_i]);
                }
            }
        }
    }

    fn drop_lost_targets(&self, unit_manager: &mut UnitManager, history: &mut HistoryManager) {
        for unit_i in 0..unit_manager.len() {
            if unit_manager.is_dead[unit_i] {
                continue;
            }
            let Some(target_i) = unit_manager.target[unit_i] else {
                continue;
            };
            let is_gone = target_i >= unit_manager.len() || unit_manager.is_dead[target_i];
            let is_other_level =
                !is_gone && unit_manager.level_pos[target_i] != unit_manager.level_pos[unit_i];
            let has_left = unit_manager.owner[unit_i] == UnitOwner::P0 && is_other_level;
            let mut is_out_of_reach = false;
            if !is_gone {
                let is_immobile = unit_manager.move_period[unit_i] == 0;
                is_out_of_reach = (is_immobile && is_other_level)
                    || is_beyond_fixed_range(unit_manager, unit_i, target_i);
            }
            if !is_gone && !has_left && !is_out_of_reach {
                continue;
            }
            unit_manager.set_target(unit_i, None, history);
            unit_manager.clear_goals(unit_i, history);
        }
    }

    fn acquire_targets(&self, unit_manager: &mut UnitManager, history: &mut HistoryManager) {
        for unit_i in 0..unit_manager.len() {
            if unit_manager.is_dead[unit_i] || unit_manager.target[unit_i].is_some() {
                continue;
            }
            let unit_type = unit_manager.unit_type[unit_i];
            if !can_attack(unit_type, &unit_manager.profile[unit_i].design)
                || !picks_own_target(unit_manager, unit_i)
            {
                continue;
            }
            let aggro_range = match unit_manager.owner[unit_i] {
                UnitOwner::P0 if unit_type.is_structure() => {
                    unit_manager.profile[unit_i].aggro_range
                }
                UnitOwner::P0 => PLAYER_AGGRO_RANGE,
                UnitOwner::P1 => unit_manager.profile[unit_i].aggro_range,
            };
            let mut range = aggro_range as i32;
            if unit_manager.move_period[unit_i] == 0 {
                range = range.min(weapon_stats(unit_manager.profile[unit_i].design.weapon).range);
            }
            if range <= 0 {
                continue;
            }
            let Some(units) = self
                .scratch_level_units
                .get(&unit_manager.level_pos[unit_i])
            else {
                continue;
            };
            let tile = unit_manager.tile_position[unit_i];
            let range_squared = range * range;
            let mut nearest = None;
            let mut nearest_distance = i32::MAX;
            for &other_i in units {
                if unit_manager.is_dead[other_i]
                    || unit_manager.owner[other_i] == unit_manager.owner[unit_i]
                {
                    continue;
                }
                if unit_manager.owner[unit_i] == UnitOwner::P0
                    && unit_type == UnitType::Soldier
                    && unit_manager.profile[other_i].design.weapon == Weapon::Unarmed
                {
                    continue;
                }
                let distance = distance_squared(tile, unit_manager.tile_position[other_i]);
                if distance > range_squared || distance >= nearest_distance {
                    continue;
                }
                nearest = Some(other_i);
                nearest_distance = distance;
            }
            if nearest.is_none() {
                continue;
            }
            unit_manager.set_target(unit_i, nearest, history);
        }
    }

    fn attack(
        &self,
        unit_manager: &mut UnitManager,
        level_manager: &mut LevelManager,
        history: &mut HistoryManager,
        audio_manager: &mut AudioManager,
    ) {
        let mut impact_sfx = mem::take(&mut unit_manager.pending_impact_sfx);
        for unit_i in 0..unit_manager.len() {
            if unit_manager.is_dead[unit_i] {
                continue;
            }
            let Some(target_i) = unit_manager.target[unit_i] else {
                continue;
            };
            if target_i >= unit_manager.len() || unit_manager.is_dead[target_i] {
                continue;
            }
            let level_pos = unit_manager.level_pos[unit_i];
            if unit_manager.level_pos[target_i] != level_pos {
                continue;
            }
            let unit_type = unit_manager.unit_type[unit_i];
            let design = unit_manager.profile[unit_i].design;
            if !can_attack(unit_type, &design) {
                continue;
            }
            let weapon = weapon_stats(design.weapon);
            let men = unit_manager.men(unit_i);
            let attack_period = unit_manager.effective_attack_period(unit_i, weapon.attack_period);
            if unit_manager.turn_count % attack_period.max(1) != 0 {
                continue;
            }
            let has_moved = unit_manager.prev_tile_position[unit_i]
                != unit_manager.tile_position[unit_i]
                || unit_manager.prev_level_pos[unit_i] != level_pos;
            if has_moved && !weapon.attacks_while_moving {
                continue;
            }
            let target_tile = unit_manager.tile_position[target_i];
            let range_squared = weapon.range * weapon.range;
            if distance_squared(unit_manager.tile_position[unit_i], target_tile) > range_squared {
                continue;
            }

            let target_men = unit_manager.men(target_i);
            let impact_sfx_start = impact_sfx.len();

            let mut men_share = MEN_PER_UNIT as i32;
            if unit_type == UnitType::Soldier {
                men_share = men as i32;
            }
            if weapon.splash_radius_squared == 0 {
                let damage =
                    scaled_damage(&weapon.damage, unit_manager.unit_type[target_i], men_share);
                deal_damage(
                    unit_manager,
                    level_manager,
                    history,
                    audio_manager,
                    &mut impact_sfx,
                    unit_i,
                    target_i,
                    damage,
                );
                aggro_on_hit(unit_manager, history, target_i, unit_i);
            } else if let Some(units) = self.scratch_level_units.get(&level_pos) {
                for &victim_i in units {
                    if unit_manager.is_dead[victim_i] {
                        continue;
                    }
                    let victim_tile = unit_manager.tile_position[victim_i];
                    if distance_squared(target_tile, victim_tile) > weapon.splash_radius_squared {
                        continue;
                    }
                    let damage = splash_damage(
                        scaled_damage(&weapon.damage, unit_manager.unit_type[victim_i], men_share),
                        target_tile,
                        victim_tile,
                    );
                    if damage == 0 {
                        continue;
                    }
                    deal_damage(
                        unit_manager,
                        level_manager,
                        history,
                        audio_manager,
                        &mut impact_sfx,
                        unit_i,
                        victim_i,
                        damage,
                    );
                    aggro_on_hit(unit_manager, history, victim_i, unit_i);
                }
            }

            unit_manager.pending_attacks.push(AttackEvent {
                attacker_i: unit_i,
                target_i,
                target_men,
                weapon: design.weapon,
                impact_sfx_start,
                impact_sfx_end: impact_sfx.len(),
            });
        }
        unit_manager.pending_impact_sfx = impact_sfx;
    }

    fn chase(&self, unit_manager: &mut UnitManager, history: &mut HistoryManager) {
        for unit_i in 0..unit_manager.len() {
            if unit_manager.is_dead[unit_i] || unit_manager.move_period[unit_i] == 0 {
                continue;
            }
            let Some(target_i) = unit_manager.target[unit_i] else {
                continue;
            };
            if target_i >= unit_manager.len() || unit_manager.is_dead[target_i] {
                continue;
            }
            let target_node = GoalNode {
                level_pos: unit_manager.level_pos[target_i],
                tile: unit_manager.tile_position[target_i],
            };
            let is_same_level = target_node.level_pos == unit_manager.level_pos[unit_i];
            if !is_same_level && unit_manager.owner[unit_i] == UnitOwner::P0 {
                continue;
            }
            let weapon = weapon_stats(unit_manager.profile[unit_i].design.weapon);
            let distance = distance_squared(unit_manager.tile_position[unit_i], target_node.tile);
            if is_same_level && distance <= weapon.range * weapon.range {
                unit_manager.clear_goals(unit_i, history);
                continue;
            }
            if unit_manager.goal_queue[unit_i].first() == Some(&target_node) {
                continue;
            }
            unit_manager.set_goal(unit_i, target_node, history);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::splash_damage;
    use macroquad::prelude::IVec2;

    #[test]
    fn splash_damage_halves_per_tile_out() {
        let target = IVec2::new(10, 10);
        assert_eq!(splash_damage(14, target, target), 14);
        assert_eq!(splash_damage(14, target, IVec2::new(11, 11)), 7);
        assert_eq!(splash_damage(14, target, IVec2::new(9, 10)), 7);
        assert_eq!(splash_damage(14, target, IVec2::new(12, 11)), 3);
        assert_eq!(splash_damage(22, target, IVec2::new(10, 8)), 5);
        assert_eq!(splash_damage(1, target, IVec2::new(11, 10)), 0);
    }
}
