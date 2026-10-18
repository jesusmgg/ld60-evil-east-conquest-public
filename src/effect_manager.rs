use macroquad::prelude::*;

use crate::{
    assets::AssetManager,
    audio_manager::AudioManager,
    camera_manager::CameraManager,
    constants::{CULL_MARGIN_PX, TILE_SIZE_X, TILE_SIZE_Y},
    enums::Direction,
    history_manager::HistoryManager,
    level::level_manager::LevelManager,
    sfx::SfxEvent,
    sprite_sheet::{sheet_frame_count, sheet_frame_rect},
    unit_design::{weapon_stats, Weapon},
    unit_manager::{snap_to_2px, AttackEvent, UnitManager},
};

const PROJECTILE_SPEED_PX: f32 = 320.0;
const EFFECT_ANIMATION_FPS: f32 = 12.0;
const PROJECTILE_ORIGIN_FRACTION_X: f32 = 0.75;
const EXPLOSION_SMALL_SPLASH_MAX: i32 = 1;
const EXPLOSION_MEDIUM_SPLASH_MAX: i32 = 4;

#[derive(Clone, Copy)]
pub enum ProjectileSprite {
    ArrowSmall,
    ArrowBig,
    Stone,
    Grenade,
    Bullet,
    Ball,
}

#[derive(Clone, Copy)]
pub enum EffectSprite {
    Hit,
    ExplosionSmall,
    ExplosionMedium,
    ExplosionLarge,
}

fn weapon_projectile(weapon: Weapon) -> Option<ProjectileSprite> {
    match weapon {
        Weapon::Unarmed
        | Weapon::Dagger
        | Weapon::Sword
        | Weapon::SwordAndShield
        | Weapon::Spear
        | Weapon::WoodRam
        | Weapon::IronRam => None,
        Weapon::Javelin | Weapon::Bow | Weapon::Crossbow => Some(ProjectileSprite::ArrowSmall),
        Weapon::Ballista => Some(ProjectileSprite::ArrowBig),
        Weapon::Catapult | Weapon::Trebuchet => Some(ProjectileSprite::Stone),
        Weapon::Grenades => Some(ProjectileSprite::Grenade),
        Weapon::HandCannon | Weapon::OrganGun => Some(ProjectileSprite::Bullet),
        Weapon::Cannon => Some(ProjectileSprite::Ball),
    }
}

fn weapon_shot_sfx(weapon: Weapon) -> Option<SfxEvent> {
    match weapon {
        Weapon::Unarmed
        | Weapon::Dagger
        | Weapon::Sword
        | Weapon::SwordAndShield
        | Weapon::Spear
        | Weapon::Grenades
        | Weapon::WoodRam
        | Weapon::IronRam => None,
        Weapon::Javelin | Weapon::Bow | Weapon::Crossbow => Some(SfxEvent::ShotArrowSmall),
        Weapon::Ballista => Some(SfxEvent::ShotArrowLarge),
        Weapon::Catapult | Weapon::Trebuchet => Some(SfxEvent::ShotStone),
        Weapon::Cannon => Some(SfxEvent::ShotCannon),
        Weapon::HandCannon | Weapon::OrganGun => Some(SfxEvent::ShotBullet),
    }
}

fn weapon_hit_sfx(weapon: Weapon) -> SfxEvent {
    match weapon {
        Weapon::Unarmed
        | Weapon::Dagger
        | Weapon::Sword
        | Weapon::SwordAndShield
        | Weapon::Spear => SfxEvent::HitMetal,
        Weapon::WoodRam => SfxEvent::HitRamWood,
        Weapon::IronRam => SfxEvent::HitRamIron,
        Weapon::Javelin | Weapon::Bow | Weapon::Crossbow => SfxEvent::HitArrowSmall,
        Weapon::Ballista => SfxEvent::HitArrowLarge,
        Weapon::HandCannon | Weapon::OrganGun => SfxEvent::HitBullet,
        Weapon::Grenades => SfxEvent::ExplosionSmall,
        Weapon::Catapult | Weapon::Trebuchet => SfxEvent::ExplosionMedium,
        Weapon::Cannon => SfxEvent::ExplosionLarge,
    }
}

fn weapon_impact(weapon: Weapon) -> EffectSprite {
    let splash_radius_squared = weapon_stats(weapon).splash_radius_squared;
    if splash_radius_squared == 0 {
        return EffectSprite::Hit;
    }
    if splash_radius_squared <= EXPLOSION_SMALL_SPLASH_MAX {
        return EffectSprite::ExplosionSmall;
    }
    if splash_radius_squared <= EXPLOSION_MEDIUM_SPLASH_MAX {
        return EffectSprite::ExplosionMedium;
    }
    EffectSprite::ExplosionLarge
}

struct DelayedSfx {
    event: SfxEvent,
    level_pos: IVec2,
    elapsed: f32,
    delay: f32,
}

struct Projectile {
    sprite: ProjectileSprite,
    impact: EffectSprite,
    target_i: usize,
    target_level_pos: IVec2,
    from: Vec2,
    to: Vec2,
    flip_x: bool,
    elapsed: f32,
    duration: f32,
}

struct Effect {
    sprite: EffectSprite,
    pos: Vec2,
    flip_x: bool,
    elapsed: f32,
}

pub struct EffectManager {
    projectiles: Vec<Projectile>,
    effects: Vec<Effect>,
    delayed_sfx: Vec<DelayedSfx>,
}

impl EffectManager {
    pub fn new() -> Self {
        Self {
            projectiles: Vec::new(),
            effects: Vec::new(),
            delayed_sfx: Vec::new(),
        }
    }

    fn push_delayed_sfx(&mut self, event: SfxEvent, level_pos: IVec2, delay: f32) {
        self.delayed_sfx.push(DelayedSfx {
            event,
            level_pos,
            elapsed: 0.0,
            delay,
        });
    }

    fn push_impact_sfx(
        &mut self,
        unit_manager: &UnitManager,
        attack: &AttackEvent,
        level_pos: IVec2,
        delay: f32,
    ) {
        for event_i in attack.impact_sfx_start..attack.impact_sfx_end {
            self.push_delayed_sfx(unit_manager.pending_impact_sfx[event_i], level_pos, delay);
        }
    }

    pub fn update(
        &mut self,
        dt: f32,
        unit_manager: &mut UnitManager,
        level_manager: &LevelManager,
        assets: &AssetManager,
        audio_manager: &mut AudioManager,
        history: &mut HistoryManager,
    ) {
        if history.is_world_rewound {
            history.is_world_rewound = false;
            self.projectiles.clear();
            self.delayed_sfx.clear();
            audio_manager.spread_sfx.clear();
        }
        for attack in &unit_manager.pending_attacks {
            if attack.attacker_i >= unit_manager.len() || attack.target_i >= unit_manager.len() {
                continue;
            }
            let target_level_pos = unit_manager.level_pos[attack.target_i];
            let attacker_center = drawn_center(unit_manager, attack.attacker_i, level_manager);
            let target_center = drawn_center(unit_manager, attack.target_i, level_manager);
            let mut flip_x = target_center.x < attacker_center.x;
            if target_center.x == attacker_center.x {
                flip_x = unit_manager.direction[attack.attacker_i] == Direction::Left;
            }
            if let Some(shot_sfx) = weapon_shot_sfx(attack.weapon) {
                audio_manager.push_sfx_in_view(level_manager, target_level_pos, shot_sfx);
            }
            let from = projectile_origin(unit_manager, attack.attacker_i, level_manager, flip_x);
            let Some(to) = unit_manager.impact_point(
                attack.target_i,
                attack.target_men,
                level_manager,
                from,
                !flip_x,
            ) else {
                self.push_impact_sfx(unit_manager, attack, target_level_pos, 0.0);
                continue;
            };
            let impact = weapon_impact(attack.weapon);
            let hit_sfx = weapon_hit_sfx(attack.weapon);
            let Some(sprite) = weapon_projectile(attack.weapon) else {
                self.effects.push(Effect {
                    sprite: impact,
                    pos: to,
                    flip_x,
                    elapsed: 0.0,
                });
                self.push_delayed_sfx(hit_sfx, target_level_pos, 0.0);
                self.push_impact_sfx(unit_manager, attack, target_level_pos, 0.0);
                continue;
            };
            let duration = from.distance(to) / PROJECTILE_SPEED_PX;
            self.projectiles.push(Projectile {
                sprite,
                impact,
                target_i: attack.target_i,
                target_level_pos,
                from,
                to,
                flip_x,
                elapsed: 0.0,
                duration,
            });
            self.push_delayed_sfx(hit_sfx, target_level_pos, duration);
            self.push_impact_sfx(unit_manager, attack, target_level_pos, duration);
        }
        unit_manager.pending_attacks.clear();
        unit_manager.pending_impact_sfx.clear();

        let mut i = 0;
        while i < self.delayed_sfx.len() {
            let delayed = &mut self.delayed_sfx[i];
            delayed.elapsed += dt;
            if delayed.elapsed < delayed.delay {
                i += 1;
                continue;
            }
            audio_manager.push_sfx_in_view(level_manager, delayed.level_pos, delayed.event);
            self.delayed_sfx.swap_remove(i);
        }

        let mut i = 0;
        while i < self.projectiles.len() {
            let projectile = &mut self.projectiles[i];
            projectile.elapsed += dt;
            let target_i = projectile.target_i;
            if target_i < unit_manager.len()
                && !unit_manager.is_dead[target_i]
                && unit_manager.level_pos[target_i] == projectile.target_level_pos
            {
                if let Some(to) = unit_manager.impact_point(
                    target_i,
                    unit_manager.men(target_i),
                    level_manager,
                    projectile.from,
                    !projectile.flip_x,
                ) {
                    projectile.to = to;
                }
            }
            if projectile.elapsed < projectile.duration {
                i += 1;
                continue;
            }
            self.effects.push(Effect {
                sprite: projectile.impact,
                pos: projectile.to,
                flip_x: projectile.flip_x,
                elapsed: 0.0,
            });
            self.projectiles.swap_remove(i);
        }

        let mut i = 0;
        while i < self.effects.len() {
            let effect = &mut self.effects[i];
            effect.elapsed += dt;
            let texture = &assets.spritesheet_effects[effect.sprite as usize];
            if effect_frame(effect.elapsed) < sheet_frame_count(texture) {
                i += 1;
                continue;
            }
            self.effects.swap_remove(i);
        }
    }

    pub fn render(&self, assets: &AssetManager, camera_manager: &CameraManager) {
        let view_rect = camera_manager.world_view_rect(CULL_MARGIN_PX);

        for projectile in &self.projectiles {
            let t = projectile.elapsed / projectile.duration;
            let pos = projectile.from.lerp(projectile.to, t);
            if !view_rect.contains(pos) {
                continue;
            }
            let texture = &assets.spritesheet_projectiles[projectile.sprite as usize];
            draw_frame(
                texture,
                effect_frame(projectile.elapsed),
                pos,
                projectile.flip_x,
            );
        }

        for effect in &self.effects {
            if !view_rect.contains(effect.pos) {
                continue;
            }
            let texture = &assets.spritesheet_effects[effect.sprite as usize];
            draw_frame(
                texture,
                effect_frame(effect.elapsed),
                effect.pos,
                effect.flip_x,
            );
        }
    }
}

fn drawn_center(unit_manager: &UnitManager, unit_i: usize, level_manager: &LevelManager) -> Vec2 {
    let world_pos = snap_to_2px(unit_manager.calculate_world_position(unit_i, level_manager));
    world_pos + vec2(TILE_SIZE_X, TILE_SIZE_Y) / 2.0
}

fn projectile_origin(
    unit_manager: &UnitManager,
    unit_i: usize,
    level_manager: &LevelManager,
    flip_x: bool,
) -> Vec2 {
    let world_pos = snap_to_2px(unit_manager.calculate_world_position(unit_i, level_manager));
    let mut x = TILE_SIZE_X * PROJECTILE_ORIGIN_FRACTION_X;
    if flip_x {
        x = TILE_SIZE_X - x;
    }
    world_pos + vec2(x, TILE_SIZE_Y / 2.0)
}

fn effect_frame(elapsed: f32) -> usize {
    (elapsed * EFFECT_ANIMATION_FPS) as usize
}

fn draw_frame(texture: &Texture2D, frame: usize, center: Vec2, flip_x: bool) {
    let source = sheet_frame_rect(texture, frame);
    let pos = (center - source.size() / 2.0).round();
    draw_texture_ex(
        texture,
        pos.x,
        pos.y,
        WHITE,
        DrawTextureParams {
            source: Some(source),
            flip_x,
            ..Default::default()
        },
    );
}
