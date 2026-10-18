use crate::{
    constants::{DEFAULT_LEVEL_TILES_X, DEFAULT_LEVEL_TILES_Y},
    debug::DebugLog,
    level::level_manager::LevelManager,
    level::tile::is_tile_obstacle,
    unit_manager::UnitManager,
};
use macroquad::miniquad::gl as miniquad_gl;
use macroquad::miniquad::RawId;
use macroquad::{prelude::*, window::get_internal_gl};

pub const FIELD_SECTORS_X: i32 = 9;
pub const FIELD_SECTORS_Y: i32 = 9;
pub const FIELD_W: usize = (FIELD_SECTORS_X * DEFAULT_LEVEL_TILES_X) as usize;
pub const FIELD_H: usize = (FIELD_SECTORS_Y * DEFAULT_LEVEL_TILES_Y) as usize;

const SECTOR_W: i32 = DEFAULT_LEVEL_TILES_X;
const SECTOR_H: i32 = DEFAULT_LEVEL_TILES_Y;

const SECTOR_COUNT: usize = (FIELD_SECTORS_X * FIELD_SECTORS_Y) as usize;

const UNIT_LIST_W: usize = 128;
const UNIT_LIST_H: usize = (FIELD_W * FIELD_H).div_ceil(UNIT_LIST_W);

const UNIT_SLOT_CAPACITY: usize = UNIT_LIST_W * UNIT_LIST_H;

const UNITS_IMAGE_W: usize = UNIT_LIST_W * 2;
const UNIT_SLOT_BYTES: usize = 8;

const UNIT_SLOT_EMPTY: u8 = 255;

const PACK_W: usize = FIELD_W.div_ceil(32);
const PACK_H: usize = FIELD_H;
const _: () = assert!((FIELD_W - WINDOW_W as usize) / 32 + 2 < PACK_W);
const PACK_WALK_ROW: usize = PACK_H;

const ADJACENCY_W: usize = 4;
const ADJACENCY_H: usize = 4;

const WINDOW_W: i32 = 64;
const WINDOW_H: i32 = 25;
const _: () = assert!(WINDOW_W - SECTOR_W >= 2 && WINDOW_H - SECTOR_H >= 2);

pub struct GpuPathManager {
    packed_image: Image,
    units_image: Image,
    packed_texture: Texture2D,
    units_texture: Texture2D,
    target: RenderTarget,
    material: Material,
    readback_bytes: Vec<u8>,
    readback_fbo: u32,

    border_target: RenderTarget,
    border_material: Material,

    sector_target: RenderTarget,
    sector_material: Material,

    next_hop_target: RenderTarget,
    next_hop_material: Material,
    unit_slot: Vec<(u32, u32)>,
    unit_slot_pass: u32,
    slot_count: usize,
}

impl GpuPathManager {
    pub fn new() -> Self {
        let packed_image = Image::gen_image_color(PACK_W as u16, (PACK_H * 2) as u16, BLANK);
        let units_image = Image::gen_image_color(UNITS_IMAGE_W as u16, UNIT_LIST_H as u16, BLANK);

        let packed_texture = Texture2D::from_image(&packed_image);
        packed_texture.set_filter(FilterMode::Nearest);

        let units_texture = Texture2D::from_image(&units_image);
        units_texture.set_filter(FilterMode::Nearest);

        let target = render_target_ex(
            UNIT_LIST_W as u32,
            UNIT_LIST_H as u32,
            RenderTargetParams {
                sample_count: 0,
                ..Default::default()
            },
        );
        target.texture.set_filter(FilterMode::Nearest);

        let version_line = if cfg!(target_arch = "wasm32") {
            "#version 300 es\nprecision highp float;\nprecision highp int;\n"
        } else {
            "#version 330 core\nprecision highp float;\n"
        };
        let mut vertex_src = String::from(version_line);
        vertex_src.push_str(ALGORITHM_VERTEX_BODY);
        let mut fragment_src = String::from(version_line);
        fragment_src.push_str(&shader_constants());
        fragment_src.push_str(ALGORITHM_FRAGMENT_BODY);

        let material = load_material(
            ShaderSource::Glsl {
                vertex: &vertex_src,
                fragment: &fragment_src,
            },
            MaterialParams {
                textures: vec!["Units".to_string(), "NextHop".to_string()],
                ..Default::default()
            },
        )
        .expect("GPU pathfinding shader failed to compile");

        let border_target = render_target_ex(
            FIELD_SECTORS_X as u32,
            FIELD_SECTORS_Y as u32,
            RenderTargetParams {
                sample_count: 0,
                ..Default::default()
            },
        );
        border_target.texture.set_filter(FilterMode::Nearest);

        let mut border_fragment_src = String::from(version_line);
        border_fragment_src.push_str(&shader_constants());
        border_fragment_src.push_str(BORDER_FRAGMENT_BODY);
        let border_material = load_material(
            ShaderSource::Glsl {
                vertex: &vertex_src,
                fragment: &border_fragment_src,
            },
            MaterialParams::default(),
        )
        .expect("GPU sector border shader failed to compile");

        let sector_target = render_target_ex(
            ADJACENCY_W as u32,
            ADJACENCY_H as u32,
            RenderTargetParams {
                sample_count: 0,
                ..Default::default()
            },
        );
        sector_target.texture.set_filter(FilterMode::Nearest);

        let mut sector_fragment_src = String::from(version_line);
        sector_fragment_src.push_str(SECTOR_FRAGMENT_BODY);
        let sector_material = load_material(
            ShaderSource::Glsl {
                vertex: &vertex_src,
                fragment: &sector_fragment_src,
            },
            MaterialParams::default(),
        )
        .expect("GPU sector adjacency shader failed to compile");

        let next_hop_target = render_target_ex(
            SECTOR_COUNT as u32,
            SECTOR_COUNT as u32,
            RenderTargetParams {
                sample_count: 0,
                ..Default::default()
            },
        );
        next_hop_target.texture.set_filter(FilterMode::Nearest);

        let mut next_hop_fragment_src = String::from(version_line);
        next_hop_fragment_src.push_str(NEXT_HOP_FRAGMENT_BODY);
        let next_hop_material = load_material(
            ShaderSource::Glsl {
                vertex: &vertex_src,
                fragment: &next_hop_fragment_src,
            },
            MaterialParams::default(),
        )
        .expect("GPU sector route shader failed to compile");

        Self {
            packed_image,
            units_image,
            packed_texture,
            units_texture,
            target,
            material,
            readback_bytes: vec![0; UNIT_LIST_W * UNIT_LIST_H * 4],
            readback_fbo: 0,

            border_target,
            border_material,
            sector_target,
            sector_material,
            next_hop_target,
            next_hop_material,
            unit_slot: Vec::new(),
            unit_slot_pass: 0,
            slot_count: 0,
        }
    }

    fn window_origin(sector: IVec2) -> (i32, i32) {
        let ox =
            (sector.x * SECTOR_W - (WINDOW_W - SECTOR_W) / 2).clamp(0, FIELD_W as i32 - WINDOW_W);
        let oy =
            (sector.y * SECTOR_H - (WINDOW_H - SECTOR_H) / 2).clamp(0, FIELD_H as i32 - WINDOW_H);
        (ox, oy)
    }

    pub fn rebuild_walkability(&mut self, level_manager: &LevelManager) {
        let walk_start = PACK_WALK_ROW * PACK_W * 4;
        self.packed_image.bytes[walk_start..].fill(0);

        assert!(self.packed_image.bytes.len() >= (PACK_WALK_ROW + PACK_H) * PACK_W * 4);
        let packed = self.packed_image.bytes.as_mut_ptr();

        for sy in 0..FIELD_SECTORS_Y {
            for sx in 0..FIELD_SECTORS_X {
                let level_pos = IVec2::new(sx, sy);
                let Some(level) = level_manager.get_level_at_pos(level_pos) else {
                    continue;
                };

                let base_x = (sx * SECTOR_W) as usize;
                let base_y = (sy * SECTOR_H) as usize;

                for ty in 0..SECTOR_H as usize {
                    unsafe {
                        set_bit_range(
                            packed,
                            PACK_WALK_ROW + base_y + ty,
                            base_x,
                            SECTOR_W as usize,
                        );
                    }
                }

                for (tile_pos, tile_type) in &level.tile_layout {
                    if !is_tile_obstacle(*tile_type) {
                        continue;
                    }
                    if tile_pos.x < 0
                        || tile_pos.y < 0
                        || tile_pos.x >= SECTOR_W
                        || tile_pos.y >= SECTOR_H
                    {
                        continue;
                    }
                    let field_x = base_x + tile_pos.x as usize;
                    let bit = field_x & 31;
                    let i = ((PACK_WALK_ROW + base_y + tile_pos.y as usize) * PACK_W
                        + (field_x >> 5))
                        * 4
                        + (bit >> 3);
                    unsafe {
                        *packed.add(i) &= !(1u8 << (bit & 7));
                    }
                }
            }
        }

        self.packed_image.bytes.copy_within(walk_start.., 0);
    }

    pub fn build_unit_pictures(&mut self, unit_manager: &UnitManager) {
        self.reset_unit_list(unit_manager.len());

        let unit_count = unit_manager.len();
        assert!(unit_manager.is_dead.len() >= unit_count);
        assert!(unit_manager.level_pos.len() >= unit_count);
        assert!(unit_manager.tile_position.len() >= unit_count);
        assert!(unit_manager.goal_queue.len() >= unit_count);
        assert!(self.unit_slot.len() >= unit_count);
        assert!(self.packed_image.bytes.len() >= PACK_H * PACK_W * 4);
        assert!(self.units_image.bytes.len() >= UNIT_SLOT_CAPACITY * UNIT_SLOT_BYTES);

        let is_dead = unit_manager.is_dead.as_ptr();
        let level_pos_all = unit_manager.level_pos.as_ptr();
        let tile_position_all = unit_manager.tile_position.as_ptr();
        let goal_queue_all = unit_manager.goal_queue.as_ptr();
        let packed = self.packed_image.bytes.as_mut_ptr();
        let units = self.units_image.bytes.as_mut_ptr();
        let unit_slot = self.unit_slot.as_mut_ptr();
        let unit_slot_pass = self.unit_slot_pass;
        let mut slot_count = self.slot_count;

        let mut unit_i = 0;
        while unit_i < unit_count {
            let this_unit = unit_i;
            unit_i += 1;
            unsafe {
                if *is_dead.add(this_unit) {
                    continue;
                }

                let level_pos = *level_pos_all.add(this_unit);
                let tile_pos = *tile_position_all.add(this_unit);
                let field_x = level_pos.x * SECTOR_W + tile_pos.x;
                let field_y = level_pos.y * SECTOR_H + tile_pos.y;
                if field_x < 0
                    || field_y < 0
                    || field_x >= FIELD_W as i32
                    || field_y >= FIELD_H as i32
                {
                    continue;
                }

                let bit = (field_x & 31) as usize;
                let byte = packed
                    .add((field_y as usize * PACK_W + (field_x >> 5) as usize) * 4 + (bit >> 3));
                *byte &= !(1u8 << (bit & 7));

                let goal_queue = &*goal_queue_all.add(this_unit);
                if goal_queue.len() == 0 {
                    continue;
                }
                let goal = *goal_queue.as_ptr();
                let goal_x = goal.level_pos.x * SECTOR_W + goal.tile.x;
                let goal_y = goal.level_pos.y * SECTOR_H + goal.tile.y;
                if goal_x < 0 || goal_y < 0 || goal_x >= FIELD_W as i32 || goal_y >= FIELD_H as i32
                {
                    continue;
                }

                if slot_count >= UNIT_SLOT_CAPACITY {
                    continue;
                }

                let slot = slot_count;
                slot_count += 1;
                *unit_slot.add(this_unit) = (unit_slot_pass, slot as u32);

                let texel = units.add(slot * UNIT_SLOT_BYTES);
                *texel = field_x as u8;
                *texel.add(1) = (field_x >> 8) as u8;
                *texel.add(2) = field_y as u8;
                *texel.add(3) = (field_y >> 8) as u8;
                *texel.add(4) = goal_x as u8;
                *texel.add(5) = (goal_x >> 8) as u8;
                *texel.add(6) = goal_y as u8;
                *texel.add(7) = (goal_y >> 8) as u8;
            }
        }

        self.slot_count = slot_count;
    }

    pub fn rebuild_for_retry(&mut self, unit_manager: &UnitManager, unit_indices: &[usize]) {
        self.packed_image
            .bytes
            .copy_within(PACK_WALK_ROW * PACK_W * 4.., 0);
        self.reset_unit_list(unit_manager.len());

        for unit_i in 0..unit_manager.len() {
            if unit_manager.is_dead[unit_i] {
                continue;
            }
            let Some((field_x, field_y)) = Self::unit_field_pos(unit_manager, unit_i) else {
                continue;
            };
            let bit = field_x & 31;
            self.packed_image.bytes[(field_y * PACK_W + (field_x >> 5)) * 4 + (bit >> 3)] &=
                !(1u8 << (bit & 7));
        }

        for &unit_i in unit_indices {
            if unit_i >= unit_manager.len() || unit_manager.is_dead[unit_i] {
                continue;
            }
            let Some((field_x, field_y)) = Self::unit_field_pos(unit_manager, unit_i) else {
                continue;
            };
            self.push_unit_slot(unit_manager, unit_i, field_x, field_y);
        }
    }

    fn unit_field_pos(unit_manager: &UnitManager, unit_i: usize) -> Option<(usize, usize)> {
        let level_pos = unit_manager.level_pos[unit_i];
        let tile_pos = unit_manager.tile_position[unit_i];
        let field_x = level_pos.x * SECTOR_W + tile_pos.x;
        let field_y = level_pos.y * SECTOR_H + tile_pos.y;
        if field_x < 0 || field_y < 0 || field_x >= FIELD_W as i32 || field_y >= FIELD_H as i32 {
            return None;
        }
        Some((field_x as usize, field_y as usize))
    }

    fn reset_unit_list(&mut self, unit_count: usize) {
        if self.unit_slot.len() < unit_count {
            self.unit_slot.resize(unit_count, (0, 0));
        }
        self.unit_slot_pass = self.unit_slot_pass.wrapping_add(1);
        if self.unit_slot_pass == 0 {
            for entry in &mut self.unit_slot {
                *entry = (0, 0);
            }
            self.unit_slot_pass = 1;
        }
        self.slot_count = 0;
    }

    fn push_unit_slot(
        &mut self,
        unit_manager: &UnitManager,
        unit_i: usize,
        field_x: usize,
        field_y: usize,
    ) {
        let Some(goal) = unit_manager.goal_queue[unit_i].first() else {
            return;
        };
        let goal_x = goal.level_pos.x * SECTOR_W + goal.tile.x;
        let goal_y = goal.level_pos.y * SECTOR_H + goal.tile.y;
        if goal_x < 0 || goal_y < 0 || goal_x >= FIELD_W as i32 || goal_y >= FIELD_H as i32 {
            return;
        }

        if self.slot_count >= UNIT_SLOT_CAPACITY {
            return;
        }

        let slot = self.slot_count;
        self.slot_count += 1;
        self.unit_slot[unit_i] = (self.unit_slot_pass, slot as u32);

        let i = slot * UNIT_SLOT_BYTES;
        self.units_image.bytes[i] = field_x as u8;
        self.units_image.bytes[i + 1] = (field_x >> 8) as u8;
        self.units_image.bytes[i + 2] = field_y as u8;
        self.units_image.bytes[i + 3] = (field_y >> 8) as u8;
        self.units_image.bytes[i + 4] = goal_x as u8;
        self.units_image.bytes[i + 5] = (goal_x >> 8) as u8;
        self.units_image.bytes[i + 6] = goal_y as u8;
        self.units_image.bytes[i + 7] = (goal_y >> 8) as u8;
    }

    fn unit_list_rows(&self) -> usize {
        self.slot_count.div_ceil(UNIT_LIST_W)
    }

    pub fn upload_walkability(&mut self) {
        let gl = unsafe { get_internal_gl() };
        gl.quad_context.texture_update_part(
            self.packed_texture.raw_miniquad_id(),
            0,
            PACK_WALK_ROW as i32,
            PACK_W as i32,
            PACK_H as i32,
            &self.packed_image.bytes[PACK_WALK_ROW * PACK_W * 4..],
        );
    }

    pub fn flush_passes(&mut self) {
        let mut gl = unsafe { get_internal_gl() };
        gl.flush();
        unsafe {
            miniquad_gl::glFlush();
        }
    }

    pub fn upload_unit_pictures(&mut self) {
        let rows = self.unit_list_rows();

        for slot in self.slot_count..rows * UNIT_LIST_W {
            self.units_image.bytes[slot * UNIT_SLOT_BYTES] = UNIT_SLOT_EMPTY;
            self.units_image.bytes[slot * UNIT_SLOT_BYTES + 1] = UNIT_SLOT_EMPTY;
        }

        let gl = unsafe { get_internal_gl() };
        gl.quad_context.texture_update_part(
            self.packed_texture.raw_miniquad_id(),
            0,
            0,
            PACK_W as i32,
            PACK_H as i32,
            &self.packed_image.bytes[..PACK_WALK_ROW * PACK_W * 4],
        );

        if rows == 0 {
            return;
        }
        gl.quad_context.texture_update_part(
            self.units_texture.raw_miniquad_id(),
            0,
            0,
            UNITS_IMAGE_W as i32,
            rows as i32,
            &self.units_image.bytes[..rows * UNIT_LIST_W * UNIT_SLOT_BYTES],
        );
    }

    pub fn run_border_pass(&mut self) {
        let camera = Camera2D {
            zoom: vec2(2.0 / FIELD_SECTORS_X as f32, 2.0 / FIELD_SECTORS_Y as f32),
            target: vec2(FIELD_SECTORS_X as f32 / 2.0, FIELD_SECTORS_Y as f32 / 2.0),
            render_target: Some(self.border_target.clone()),
            ..Default::default()
        };
        set_camera(&camera);
        gl_use_material(&self.border_material);
        draw_texture_ex(
            &self.packed_texture,
            0.0,
            0.0,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(FIELD_SECTORS_X as f32, FIELD_SECTORS_Y as f32)),
                ..Default::default()
            },
        );
        gl_use_default_material();
    }

    pub fn run_sector_pass(&mut self) {
        let camera = Camera2D {
            zoom: vec2(2.0 / ADJACENCY_W as f32, 2.0 / ADJACENCY_H as f32),
            target: vec2(ADJACENCY_W as f32 / 2.0, ADJACENCY_H as f32 / 2.0),
            render_target: Some(self.sector_target.clone()),
            ..Default::default()
        };
        set_camera(&camera);
        gl_use_material(&self.sector_material);
        draw_texture_ex(
            &self.border_target.texture,
            0.0,
            0.0,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(ADJACENCY_W as f32, ADJACENCY_H as f32)),
                ..Default::default()
            },
        );
        gl_use_default_material();
    }

    pub fn run_next_hop_pass(&mut self) {
        let camera = Camera2D {
            zoom: vec2(2.0 / SECTOR_COUNT as f32, 2.0 / SECTOR_COUNT as f32),
            target: vec2(SECTOR_COUNT as f32 / 2.0, SECTOR_COUNT as f32 / 2.0),
            render_target: Some(self.next_hop_target.clone()),
            ..Default::default()
        };
        set_camera(&camera);
        clear_background(BLANK);
        gl_use_material(&self.next_hop_material);
        draw_texture_ex(
            &self.sector_target.texture,
            0.0,
            0.0,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(SECTOR_COUNT as f32, SECTOR_COUNT as f32)),
                ..Default::default()
            },
        );
        gl_use_default_material();
    }

    pub fn run_pass(&mut self) {
        let rows = self.unit_list_rows();
        if rows == 0 {
            return;
        }

        self.material
            .set_texture("Units", self.units_texture.clone());
        self.material
            .set_texture("NextHop", self.next_hop_target.texture.clone());

        let camera = Camera2D {
            zoom: vec2(2.0 / UNIT_LIST_W as f32, 2.0 / UNIT_LIST_H as f32),
            target: vec2(UNIT_LIST_W as f32 / 2.0, UNIT_LIST_H as f32 / 2.0),
            render_target: Some(self.target.clone()),
            ..Default::default()
        };
        set_camera(&camera);
        gl_use_material(&self.material);
        draw_texture_ex(
            &self.packed_texture,
            0.0,
            0.0,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(UNIT_LIST_W as f32, rows as f32)),
                ..Default::default()
            },
        );
        gl_use_default_material();
        set_default_camera();
    }

    pub fn finish(&mut self) {
        unsafe {
            miniquad_gl::glFinish();
        }
    }

    pub fn readback(&mut self) {
        let rows = self.unit_list_rows();
        if rows == 0 {
            return;
        }

        let gl = unsafe { get_internal_gl() };

        if self.readback_fbo == 0 {
            let raw = unsafe {
                gl.quad_context
                    .texture_raw_id(self.target.texture.raw_miniquad_id())
            };
            let RawId::OpenGl(texture_name) = raw;
            unsafe {
                miniquad_gl::glGenFramebuffers(1, &mut self.readback_fbo);
                miniquad_gl::glBindFramebuffer(miniquad_gl::GL_FRAMEBUFFER, self.readback_fbo);
                miniquad_gl::glFramebufferTexture2D(
                    miniquad_gl::GL_FRAMEBUFFER,
                    miniquad_gl::GL_COLOR_ATTACHMENT0,
                    miniquad_gl::GL_TEXTURE_2D,
                    texture_name,
                    0,
                );
                miniquad_gl::glBindFramebuffer(miniquad_gl::GL_FRAMEBUFFER, 0);
            }
        }

        unsafe {
            let mut previous_fbo: i32 = 0;
            miniquad_gl::glGetIntegerv(miniquad_gl::GL_DRAW_FRAMEBUFFER_BINDING, &mut previous_fbo);
            miniquad_gl::glBindFramebuffer(miniquad_gl::GL_FRAMEBUFFER, self.readback_fbo);
            miniquad_gl::glReadPixels(
                0,
                0,
                UNIT_LIST_W as i32,
                rows as i32,
                miniquad_gl::GL_RGBA,
                miniquad_gl::GL_UNSIGNED_BYTE,
                self.readback_bytes.as_mut_ptr() as *mut _,
            );
            miniquad_gl::glBindFramebuffer(miniquad_gl::GL_FRAMEBUFFER, previous_fbo as u32);
        }
    }

    pub fn goal_reachable_for_unit(&self, unit_i: usize) -> bool {
        self.unit_nibble(unit_i) & 8 == 0
    }

    fn unit_nibble(&self, unit_i: usize) -> u8 {
        if unit_i >= self.unit_slot.len() {
            return 0;
        }
        let (pass, slot) = self.unit_slot[unit_i];
        if pass != self.unit_slot_pass {
            return 0;
        }
        self.readback_bytes[slot as usize * 4]
    }

    pub fn direction_for_unit(&self, unit_i: usize) -> u8 {
        self.unit_nibble(unit_i) & 7
    }

    fn is_walkable(&self, field_x: usize, field_y: usize) -> bool {
        self.packed_bit(PACK_WALK_ROW + field_y, field_x)
    }

    fn is_passable(&self, field_x: usize, field_y: usize) -> bool {
        self.packed_bit(field_y, field_x)
    }

    fn packed_bit(&self, row: usize, field_x: usize) -> bool {
        let bit = field_x & 31;
        let i = (row * PACK_W + (field_x >> 5)) * 4 + (bit >> 3);
        self.packed_image.bytes[i] & (1u8 << (bit & 7)) != 0
    }

    pub fn build_and_run(&mut self, level_manager: &LevelManager, unit_manager: &UnitManager) {
        self.rebuild_walkability(level_manager);
        self.upload_walkability();
        self.run_border_pass();
        self.run_sector_pass();
        self.run_next_hop_pass();
        self.flush_passes();
        self.build_unit_pictures(unit_manager);
        self.upload_unit_pictures();
        self.run_pass();
        self.readback();
    }

    pub fn log_unit_window(
        &mut self,
        debug_log: &mut DebugLog,
        unit_manager: &UnitManager,
        unit_i: usize,
    ) {
        let level_pos = unit_manager.level_pos[unit_i];
        let tile_pos = unit_manager.tile_position[unit_i];
        let unit_fx = level_pos.x * SECTOR_W + tile_pos.x;
        let unit_fy = level_pos.y * SECTOR_H + tile_pos.y;
        let (ox, oy) = Self::window_origin(level_pos);

        let Some(goal) = unit_manager.goal_queue[unit_i].first().copied() else {
            debug_log.log(format!("gpuunit {unit_i}: no goal"));
            return;
        };

        let dir_code = self.direction_for_unit(unit_i);
        let reachable = self.goal_reachable_for_unit(unit_i);

        let goal_fx = goal.level_pos.x * SECTOR_W + goal.tile.x;
        let goal_fy = goal.level_pos.y * SECTOR_H + goal.tile.y;
        let goal_walkable = if self.is_walkable(goal_fx as usize, goal_fy as usize) {
            "yes"
        } else {
            "no"
        };

        debug_log.log(format!(
            "gpuunit {} at {},{}:{},{} field={},{} window={},{} {}x{} goal={},{}:{},{} field={},{} goal_walkable={} dir_code={} reachable={}",
            unit_i,
            level_pos.x,
            level_pos.y,
            tile_pos.x,
            tile_pos.y,
            unit_fx,
            unit_fy,
            ox,
            oy,
            WINDOW_W,
            WINDOW_H,
            goal.level_pos.x,
            goal.level_pos.y,
            goal.tile.x,
            goal.tile.y,
            goal_fx,
            goal_fy,
            goal_walkable,
            dir_code,
            reachable
        ));

        for row in 0..WINDOW_H {
            let mut line = String::with_capacity(WINDOW_W as usize);
            for col in 0..WINDOW_W {
                let fx = ox + col;
                let fy = oy + row;
                let walkable = self.is_walkable(fx as usize, fy as usize);
                let occupied = walkable && !self.is_passable(fx as usize, fy as usize);

                let is_unit = fx == unit_fx && fy == unit_fy;
                let is_goal = fx == goal_fx && fy == goal_fy;

                let glyph = if is_unit {
                    'U'
                } else if is_goal {
                    'G'
                } else if !walkable {
                    '#'
                } else if occupied {
                    'o'
                } else {
                    '.'
                };
                line.push(glyph);
            }
            debug_log.log(format!("gpuunit row {:2} {}", oy + row, line));
        }
    }

    pub fn log_stats(&self, debug_log: &mut DebugLog, unit_manager: &UnitManager) {
        let mut walkable_count = 0usize;
        let mut occupied_count = 0usize;

        for field_y in 0..FIELD_H {
            for field_x in 0..FIELD_W {
                if self.is_walkable(field_x, field_y) {
                    walkable_count += 1;
                    if !self.is_passable(field_x, field_y) {
                        occupied_count += 1;
                    }
                }
            }
        }

        let goal_count = self.slot_count;

        let mut open_sectors = 0usize;
        for sy in 0..FIELD_SECTORS_Y {
            for sx in 0..FIELD_SECTORS_X {
                let base_x = (sx * SECTOR_W) as usize;
                let base_y = (sy * SECTOR_H) as usize;
                let mut any_open = false;
                for ty in 0..SECTOR_H as usize {
                    for tx in 0..SECTOR_W as usize {
                        if self.is_walkable(base_x + tx, base_y + ty) {
                            any_open = true;
                            break;
                        }
                    }
                    if any_open {
                        break;
                    }
                }
                if any_open {
                    open_sectors += 1;
                }
            }
        }

        let unit_count = unit_manager.len();
        debug_log.log(format!(
            "gpufield: {}x{} sectors={} walkable={} occupied={} goals={} units={}",
            FIELD_W, FIELD_H, open_sectors, walkable_count, occupied_count, goal_count, unit_count
        ));
    }
}

unsafe fn set_bit_range(packed: *mut u8, row: usize, first_x: usize, count: usize) {
    let end = first_x + count;
    let mut x = first_x;
    while x < end {
        let chunk = x >> 5;
        let chunk_end = ((chunk + 1) << 5).min(end);
        let bits = chunk_end - x;
        let mask = if bits == 32 {
            u32::MAX
        } else {
            ((1u32 << bits) - 1) << (x & 31)
        };
        let i = (row * PACK_W + chunk) * 4;
        *packed.add(i) |= mask as u8;
        *packed.add(i + 1) |= (mask >> 8) as u8;
        *packed.add(i + 2) |= (mask >> 16) as u8;
        *packed.add(i + 3) |= (mask >> 24) as u8;
        x = chunk_end;
    }
}

fn shader_constants() -> String {
    format!(
        "const int FIELD_W = {};\nconst int FIELD_H = {};\nconst int SECTORS_X = {};\n\
         const int SECTORS_Y = {};\nconst int SECTOR_W = {};\nconst int SECTOR_H = {};\n\
         const int WINDOW_W = {};\nconst int WINDOW_H = {};\n",
        FIELD_W, FIELD_H, FIELD_SECTORS_X, FIELD_SECTORS_Y, SECTOR_W, SECTOR_H, WINDOW_W, WINDOW_H
    )
}

const ALGORITHM_VERTEX_BODY: &str = r#"
in vec3 position;

uniform mat4 Model;
uniform mat4 Projection;

void main() {
    gl_Position = Projection * Model * vec4(position, 1);
}
"#;

const NEXT_HOP_FRAGMENT_BODY: &str = r#"
out vec4 frag_color;

uniform sampler2D Texture;

void main() {
    ivec2 pair = ivec2(gl_FragCoord.xy);
    int from_sector = pair.x;
    int to_sector = pair.y;

    if (from_sector == to_sector) {
        frag_color = vec4(0.0, 0.0, 0.0, 1.0);
        return;
    }

    uint mask[12];
    for (int d = 0; d < 4; d++) {
        uint w0 = 0u; uint w1 = 0u; uint w2 = 0u;
        for (int t = 0; t < 4; t++) {
            vec4 texel = texelFetch(Texture, ivec2(t, d), 0);
            uint chunk = uint(texel.r * 255.0 + 0.5)
                       | (uint(texel.g * 255.0 + 0.5) << 8u)
                       | (uint(texel.b * 255.0 + 0.5) << 16u);
            int shift = t * 24;
            if (shift < 32) {
                w0 |= chunk << uint(shift);
                if (shift > 8) w1 |= chunk >> uint(32 - shift);
            } else if (shift < 64) {
                w1 |= chunk << uint(shift - 32);
                if (shift - 32 > 8) w2 |= chunk >> uint(64 - shift);
            } else {
                w2 |= chunk << uint(shift - 64);
            }
        }
        mask[d * 3] = w0;
        mask[d * 3 + 1] = w1;
        mask[d * 3 + 2] = w2;
    }

    uint up0 = mask[0]; uint up1 = mask[1]; uint up2 = mask[2];
    uint rt0 = mask[3]; uint rt1 = mask[4]; uint rt2 = mask[5];
    uint dn0 = mask[6]; uint dn1 = mask[7]; uint dn2 = mask[8];
    uint lf0 = mask[9]; uint lf1 = mask[10]; uint lf2 = mask[11];

    int fword = from_sector / 32;
    uint fbit = 1u << uint(from_sector - fword * 32);
    int nb[4];
    nb[0] = ((mask[fword] & fbit) != 0u) ? from_sector - 9 : -1;
    nb[1] = ((mask[6 + fword] & fbit) != 0u) ? from_sector + 9 : -1;
    nb[2] = ((mask[9 + fword] & fbit) != 0u) ? from_sector - 1 : -1;
    nb[3] = ((mask[3 + fword] & fbit) != 0u) ? from_sector + 1 : -1;

    uint f0 = 0u; uint f1 = 0u; uint f2 = 0u;
    int gword = to_sector / 32;
    uint gbit = 1u << uint(to_sector - gword * 32);
    if (gword == 0) f0 = gbit; else if (gword == 1) f1 = gbit; else f2 = gbit;

    int hop_dist[4];
    for (int d = 0; d < 4; d++) hop_dist[d] = -1;

    for (int round = 0; round < 82; round++) {
        for (int d = 0; d < 4; d++) {
            if (nb[d] < 0 || hop_dist[d] >= 0) continue;
            int w = nb[d] / 32;
            uint b = 1u << uint(nb[d] - w * 32);
            uint word_bits = (w == 0) ? f0 : ((w == 1) ? f1 : f2);
            if ((word_bits & b) != 0u) hop_dist[d] = round;
        }

        uint t0; uint t1; uint t2;
        uint n0; uint n1; uint n2;

        t0 = f0 & dn0; t1 = f1 & dn1; t2 = f2 & dn2;
        n0 = t0 << 9u;
        n1 = (t1 << 9u) | (t0 >> 23u);
        n2 = (t2 << 9u) | (t1 >> 23u);

        t0 = f0 & up0; t1 = f1 & up1; t2 = f2 & up2;
        n0 |= (t0 >> 9u) | (t1 << 23u);
        n1 |= (t1 >> 9u) | (t2 << 23u);
        n2 |= (t2 >> 9u);

        t0 = f0 & rt0; t1 = f1 & rt1; t2 = f2 & rt2;
        n0 |= t0 << 1u;
        n1 |= (t1 << 1u) | (t0 >> 31u);
        n2 |= (t2 << 1u) | (t1 >> 31u);

        t0 = f0 & lf0; t1 = f1 & lf1; t2 = f2 & lf2;
        n0 |= (t0 >> 1u) | (t1 << 31u);
        n1 |= (t1 >> 1u) | (t2 << 31u);
        n2 |= (t2 >> 1u);

        n0 |= f0; n1 |= f1; n2 |= f2;
        if (n0 == f0 && n1 == f1 && n2 == f2) break;
        f0 = n0; f1 = n1; f2 = n2;
    }

    int hop = -1;
    int best = 9999;
    for (int d = 0; d < 4; d++) {
        if (hop_dist[d] >= 0 && hop_dist[d] < best) {
            best = hop_dist[d];
            hop = d;
        }
    }

    frag_color = vec4(float(hop + 1) * 51.0 / 255.0, 0.0, 0.0, 1.0);
}
"#;

const BORDER_FRAGMENT_BODY: &str = r#"
out vec4 frag_color;

uniform sampler2D Texture;

uint packed_word(int row, int chunk) {
    vec4 t = texelFetch(Texture, ivec2(chunk, row), 0);
    return uint(t.r * 255.0 + 0.5)
         | (uint(t.g * 255.0 + 0.5) << 8u)
         | (uint(t.b * 255.0 + 0.5) << 16u)
         | (uint(t.a * 255.0 + 0.5) << 24u);
}

bool walkable_at(int x, int y) {
    return (packed_word(FIELD_H + y, x >> 5) & (1u << uint(x & 31))) != 0u;
}

void main() {
    int sx = int(gl_FragCoord.x);
    int sy = int(gl_FragCoord.y);
    int bx = sx * SECTOR_W;
    int by = sy * SECTOR_H;

    bool up = false;
    bool down = false;
    for (int i = 0; i < SECTOR_W; i++) {
        if (sy > 0 && walkable_at(bx + i, by) && walkable_at(bx + i, by - 1)) {
            up = true;
        }
        if (sy < SECTORS_Y - 1
            && walkable_at(bx + i, by + SECTOR_H - 1)
            && walkable_at(bx + i, by + SECTOR_H)) {
            down = true;
        }
    }

    bool left = false;
    bool right = false;
    for (int i = 0; i < SECTOR_H; i++) {
        if (sx > 0 && walkable_at(bx, by + i) && walkable_at(bx - 1, by + i)) {
            left = true;
        }
        if (sx < SECTORS_X - 1
            && walkable_at(bx + SECTOR_W - 1, by + i)
            && walkable_at(bx + SECTOR_W, by + i)) {
            right = true;
        }
    }

    frag_color = vec4(
        up ? 1.0 : 0.0,
        right ? 1.0 : 0.0,
        down ? 1.0 : 0.0,
        left ? 1.0 : 0.0
    );
}
"#;

const SECTOR_FRAGMENT_BODY: &str = r#"
out vec4 frag_color;

uniform sampler2D Texture;

void main() {
    int texel = int(gl_FragCoord.x);
    int d = int(gl_FragCoord.y);

    int base = texel * 24;
    int channel[3];
    for (int c = 0; c < 3; c++) {
        int bits = 0;
        for (int b = 0; b < 8; b++) {
            int s = base + c * 8 + b;
            if (s < 81) {
                int sy = s / 9;
                vec4 t = texelFetch(Texture, ivec2(s - sy * 9, sy), 0);
                float open = (d == 0) ? t.r : ((d == 1) ? t.g : ((d == 2) ? t.b : t.a));
                if (open > 0.5) bits |= (1 << b);
            }
        }
        channel[c] = bits;
    }

    frag_color = vec4(
        float(channel[0]) / 255.0,
        float(channel[1]) / 255.0,
        float(channel[2]) / 255.0,
        1.0
    );
}
"#;

const ALGORITHM_FRAGMENT_BODY: &str = r#"
out vec4 frag_color;

uniform sampler2D Texture;
uniform sampler2D Units;
uniform sampler2D NextHop;

uint packed_word(int row, int chunk) {
    vec4 t = texelFetch(Texture, ivec2(chunk, row), 0);
    return uint(t.r * 255.0 + 0.5)
         | (uint(t.g * 255.0 + 0.5) << 8u)
         | (uint(t.b * 255.0 + 0.5) << 16u)
         | (uint(t.a * 255.0 + 0.5) << 24u);
}

bool walkable_at(int x, int y) {
    return (packed_word(FIELD_H + y, x >> 5) & (1u << uint(x & 31))) != 0u;
}

uint col_lo(int x) {
    return (x >= 0 && x < 32) ? (1u << uint(x)) : 0u;
}

uint col_hi(int x) {
    return (x >= 32 && x < 64) ? (1u << uint(x - 32)) : 0u;
}

bool pair_open(ivec2 a, ivec2 b) {
    return walkable_at(a.x, a.y) && walkable_at(b.x, b.y);
}

int solve_unit(int tx, int ty, int goal_x, int goal_y) {

    int usx = tx / SECTOR_W;
    int usy = ty / SECTOR_H;
    int us = usy * SECTORS_X + usx;
    int gs = (goal_y / SECTOR_H) * SECTORS_X + (goal_x / SECTOR_W);

    int wp_x = goal_x;
    int wp_y = goal_y;

    if (us != gs) {
        int hop = int(texelFetch(NextHop, ivec2(us, gs), 0).r * 255.0 + 25.0) / 51 - 1;

        if (hop < 0) {
            return 8;
        }

        int bx = usx * SECTOR_W;
        int by = usy * SECTOR_H;
        bool found_door = false;
        if (hop == 0) {
            for (int i = 0; i < SECTOR_W; i++) {
                if (pair_open(ivec2(bx + i, by), ivec2(bx + i, by - 1))) {
                    wp_x = bx + i; wp_y = by - 1; found_door = true; break;
                }
            }
        } else if (hop == 1) {
            for (int i = 0; i < SECTOR_W; i++) {
                if (pair_open(ivec2(bx + i, by + SECTOR_H - 1), ivec2(bx + i, by + SECTOR_H))) {
                    wp_x = bx + i; wp_y = by + SECTOR_H; found_door = true; break;
                }
            }
        } else if (hop == 2) {
            for (int i = 0; i < SECTOR_H; i++) {
                if (pair_open(ivec2(bx, by + i), ivec2(bx - 1, by + i))) {
                    wp_x = bx - 1; wp_y = by + i; found_door = true; break;
                }
            }
        } else {
            for (int i = 0; i < SECTOR_H; i++) {
                if (pair_open(ivec2(bx + SECTOR_W - 1, by + i), ivec2(bx + SECTOR_W, by + i))) {
                    wp_x = bx + SECTOR_W; wp_y = by + i; found_door = true; break;
                }
            }
        }
        if (!found_door) {
            return 8;
        }
    }

    int gx_abs = wp_x;
    int gy_abs = wp_y;

    int ox = clamp(
        (tx / SECTOR_W) * SECTOR_W - (WINDOW_W - SECTOR_W) / 2, 0, FIELD_W - WINDOW_W);
    int oy = clamp(
        (ty / SECTOR_H) * SECTOR_H - (WINDOW_H - SECTOR_H) / 2, 0, FIELD_H - WINDOW_H);
    int lx = tx - ox;
    int ly = ty - oy;

    int gx = gx_abs - ox;
    int gy = gy_abs - oy;

    if (gx < 0 || gx >= WINDOW_W || gy < 0 || gy >= WINDOW_H) {
        return 8;
    }

    int chunk = ox >> 5;
    int off = ox & 31;

    uint free_lo[WINDOW_H];
    uint free_hi[WINDOW_H];
    for (int i = 0; i < WINDOW_H; i++) {
        int y = oy + i;
        uint w0 = packed_word(y, chunk);
        uint w1 = packed_word(y, chunk + 1);
        if (off == 0) {
            free_lo[i] = w0;
            free_hi[i] = w1;
        } else {
            uint w2 = packed_word(y, chunk + 2);
            free_lo[i] = (w0 >> uint(off)) | (w1 << uint(32 - off));
            free_hi[i] = (w1 >> uint(off)) | (w2 << uint(32 - off));
        }
    }

    bool goal_walkable = walkable_at(gx_abs, gy_abs);

    for (int i = 0; i < WINDOW_H; i++) {
        if (i == gy && goal_walkable) {
            free_lo[i] |= col_lo(gx);
            free_hi[i] |= col_hi(gx);
        }
        if (i == ly) {
            free_lo[i] &= ~col_lo(lx);
            free_hi[i] &= ~col_hi(lx);
        }
    }

    uint col_bit_lo = col_lo(lx);
    uint col_bit_hi = col_hi(lx);
    uint left_bit_lo = col_lo(lx - 1);
    uint left_bit_hi = col_hi(lx - 1);
    uint right_bit_lo = col_lo(lx + 1);
    uint right_bit_hi = col_hi(lx + 1);
    int up_row = ly - 1;
    int down_row = ly + 1;

    if (gx == lx && gy == up_row) return 1;
    if (gx == lx && gy == down_row) return 2;
    if (gy == ly && lx > 0 && gx == lx - 1) return 3;
    if (gy == ly && lx < WINDOW_W - 1 && gx == lx + 1) return 4;

    uint reach_lo[WINDOW_H];
    uint reach_hi[WINDOW_H];
    bool goal_free = false;
    for (int i = 0; i < WINDOW_H; i++) {
        reach_lo[i] = (i == gy) ? col_lo(gx) : 0u;
        reach_hi[i] = (i == gy) ? col_hi(gx) : 0u;
        if (i == gy) {
            goal_free = ((free_lo[i] & col_lo(gx)) | (free_hi[i] & col_hi(gx))) != 0u;
        }
    }

    if (!goal_free) {
        return 8;
    }

    for (int step = 0; step < WINDOW_W * WINDOW_H; step++) {
        bool grew = false;
        bool hit_up = false;
        bool hit_down = false;
        bool hit_left = false;
        bool hit_right = false;

        uint above_lo = 0u;
        uint above_hi = 0u;
        for (int i = 0; i < WINDOW_H; i++) {
            uint r_lo = reach_lo[i];
            uint r_hi = reach_hi[i];
            uint grown_lo = r_lo | (r_lo >> 1u) | (r_lo << 1u) | (r_hi << 31u) | above_lo;
            uint grown_hi = r_hi | (r_hi >> 1u) | (r_hi << 1u) | (r_lo >> 31u) | above_hi;
            if (i < WINDOW_H - 1) {
                grown_lo |= reach_lo[i + 1];
                grown_hi |= reach_hi[i + 1];
            }
            uint n_lo = grown_lo & free_lo[i];
            uint n_hi = grown_hi & free_hi[i];
            above_lo = r_lo;
            above_hi = r_hi;
            reach_lo[i] = n_lo;
            reach_hi[i] = n_hi;
            if (n_lo != r_lo || n_hi != r_hi) grew = true;

            if (i == up_row) {
                hit_up = ((n_lo & col_bit_lo) | (n_hi & col_bit_hi)) != 0u;
            }
            if (i == down_row) {
                hit_down = ((n_lo & col_bit_lo) | (n_hi & col_bit_hi)) != 0u;
            }
            if (i == ly) {
                hit_left = ((n_lo & left_bit_lo) | (n_hi & left_bit_hi)) != 0u;
                hit_right = ((n_lo & right_bit_lo) | (n_hi & right_bit_hi)) != 0u;
            }
        }

        if (hit_up) return 1;
        if (hit_down) return 2;
        if (hit_left) return 3;
        if (hit_right) return 4;

        if (!grew) {
            return 8;
        }
    }

    return 8;
}

int channel_pair(float low, float high) {
    return int(low * 255.0 + 0.5) | (int(high * 255.0 + 0.5) << 8);
}

void main() {
    ivec2 slot = ivec2(gl_FragCoord.xy);
    vec4 at = texelFetch(Units, ivec2(slot.x * 2, slot.y), 0);

    int tx = channel_pair(at.r, at.g);
    if (tx >= FIELD_W) {
        frag_color = vec4(0.0, 0.0, 0.0, 1.0);
        return;
    }
    vec4 goal = texelFetch(Units, ivec2(slot.x * 2 + 1, slot.y), 0);
    int ty = channel_pair(at.b, at.a);
    int goal_x = channel_pair(goal.r, goal.g);
    int goal_y = channel_pair(goal.b, goal.a);

    int value = solve_unit(tx, ty, goal_x, goal_y);
    frag_color = vec4(float(value) / 255.0, 0.0, 0.0, 1.0);
}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    fn bit_is_set(packed: &[u8], row: usize, x: usize) -> bool {
        let i = (row * PACK_W + (x >> 5)) * 4 + ((x & 31) >> 3);
        packed[i] & (1 << (x & 7)) != 0
    }

    fn assert_only_range_set(first_x: usize, count: usize) {
        let mut packed = vec![0u8; PACK_W * 2 * 4];
        unsafe {
            set_bit_range(packed.as_mut_ptr(), 1, first_x, count);
        }
        for x in 0..PACK_W * 32 {
            let expected = x >= first_x && x < first_x + count;
            assert_eq!(bit_is_set(&packed, 1, x), expected, "column {}", x);
            assert!(!bit_is_set(&packed, 0, x), "row 0 column {}", x);
        }
    }

    #[test]
    fn bit_range_inside_one_texel() {
        assert_only_range_set(3, 10);
    }

    #[test]
    fn bit_range_across_two_texels() {
        assert_only_range_set(24, 20);
    }

    #[test]
    fn bit_range_across_three_texels() {
        assert_only_range_set(31, 40);
    }

    #[test]
    fn bit_range_keeps_bits_already_set() {
        let mut packed = vec![0u8; PACK_W * 4];
        unsafe {
            set_bit_range(packed.as_mut_ptr(), 0, 0, 40);
            set_bit_range(packed.as_mut_ptr(), 0, 40, 40);
        }
        for x in 0..80 {
            assert!(bit_is_set(&packed, 0, x), "column {}", x);
        }
        assert!(!bit_is_set(&packed, 0, 80));
    }
}
