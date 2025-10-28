use std::{collections::HashMap, ops::Range};

use crate::game::{
    Side,
    boxes::{BlockType, CollisionBox, HitBox, HurtBox},
    character::{self, EndBehavior, MoveInput, StartBehavior, StateData, StateFlags},
    input::{ButtonFlag, RelativeDirection, RelativeMotion},
    render::animation::{Animation, AnimationLayout},
};

use sdl3::{
    render::{FPoint, FRect, Texture, TextureCreator},
    video::WindowContext,
};
use serde::Deserialize;

pub fn deserialize<'a>(
    texture_creator: &'a TextureCreator<WindowContext>,
    global_textures: &mut Vec<Texture<'a>>,
    config: &str,
    start_pos: FPoint,
    start_side: Side,
) -> Result<(character::Context, character::State), String> {
    let src = std::fs::read_to_string(config)
        .map_err(|err| format!("Failed to open: '{config}': {err}"))?;
    let character_json: CharacterJson = serde_json::from_str(&src)
        .map_err(|err| format!("Failed to parse: '{config}': {err}"))?;

    let mut animation_data = Vec::new();
    for mov in &character_json.moves {
        let new_animation = Animation::load(
            texture_creator,
            global_textures,
            &mov.animation.texture_path,
            mov.animation.w,
            mov.animation.h,
            mov.animation.frames,
            mov.animation.layout.to_animation_layout(),
        )?;

        animation_data.push(new_animation);
    }

    let move_names_to_pos: HashMap<_, _> = character_json
        .moves
        .iter()
        .enumerate()
        .map(|(i, mov)| (mov.name.as_str(), i))
        .collect();

    let mut hit_box_data = Vec::new();
    let mut hurt_box_data = Vec::new();
    let mut collision_box_data = Vec::new();

    let mut inputs = Vec::new();
    let mut hit_boxes_start = Vec::new();
    let mut hurt_boxes_start = Vec::new();
    let mut start_behaviors = Vec::new();
    let mut flags = Vec::new();
    let mut end_behaviors = Vec::new();
    let mut cancel_options = Vec::new();
    let mut cancel_windows = Vec::new();

    let mut run_length_hit_boxes = Vec::new();
    let mut run_length_hurt_boxes = Vec::new();
    let mut run_length_cancel_options = Vec::new();

    let mut hit_box_offset = 0usize;
    let mut hurt_box_offset = 0usize;
    let mut cancel_options_offset = 0usize;
    for mov in &character_json.moves {
        append_hit_box_data(
            mov,
            &mut hit_box_data,
            &mut run_length_hit_boxes,
            &mut hit_boxes_start,
            &mut hit_box_offset,
        )?;
        append_hurt_box_data(
            mov,
            &mut hurt_box_data,
            &mut run_length_hurt_boxes,
            &mut hurt_boxes_start,
            &mut hurt_box_offset,
        )?;
        append_cancel_options_data(
            mov,
            &move_names_to_pos,
            &mut cancel_options,
            &mut run_length_cancel_options,
            &mut cancel_options_offset,
        )?;
        inputs.push(mov.input.to_move_input());
        collision_box_data.push(mov.collision_box.to_collision_box());
        start_behaviors.push(mov.start_behavior.to_start_behavior());

        let conv_end_beh = mov
            .end_behavior
            .to_end_behavior(&move_names_to_pos)
            .map_err(|missing_move| {
                format!(
                    "Move '{}', EndBehavior: Could not found move '{}'",
                    mov.name, missing_move
                )
            })?;
        end_behaviors.push(conv_end_beh);

        let conv_flags = mov.flags.iter().fold(StateFlags::NONE, |flags, next| {
            flags.union(next.to_state_json())
        });
        flags.push(conv_flags);

        let conv_cancel_range = mov.cancel_window.to_range();
        cancel_windows.push(conv_cancel_range);
    }

    let Some(&block_stun_state) = move_names_to_pos.get(character_json.block_stun_state.as_str())
    else {
        return Err(format!(
            "Invalid block_stun_state: '{}'",
            character_json.ground_hit_state
        ));
    };
    let Some(&ground_hit_state) = move_names_to_pos.get(character_json.ground_hit_state.as_str())
    else {
        return Err(format!(
            "Invalid ground_hit_state: '{}'",
            character_json.ground_hit_state
        ));
    };
    let Some(&launch_hit_state) = move_names_to_pos.get(character_json.launch_hit_state.as_str())
    else {
        return Err(format!(
            "Invalid launch_hit_state: '{}'",
            character_json.launch_hit_state
        ));
    };

    let context = character::Context {
        name: character_json.name,
        max_hp: character_json.hp as f32,
        start_pos,
        start_side,
        block_stun_state,
        ground_hit_state,
        launch_hit_state,
        states: StateData {
            inputs,
            cancel_windows,
            cancel_options,
            hit_boxes_start,
            hurt_boxes_start,
            flags,
            start_behaviors,
            end_behaviors,
            run_length_hit_boxes,
            run_length_hurt_boxes,
            run_length_cancel_options,
            hit_box_data,
            hurt_box_data,
            collision_box_data,
            animation_data,
        },
    };
    let state = character::State::new(character_json.hp as f32, start_pos, start_side);

    Ok((context, state))
}

fn append_hit_box_data(
    mov: &MoveJson,
    hit_box_data: &mut Vec<HitBox>,
    run_length_hit_boxes: &mut Vec<(usize, Range<usize>)>,
    hit_boxes_start: &mut Vec<usize>,
    offset: &mut usize,
) -> Result<(), String> {
    hit_boxes_start.push(run_length_hit_boxes.len());

    for pair in mov.hit_boxes.windows(2) {
        let first = &pair[0];
        let second = &pair[1];
        let duration = second.frame.checked_sub(first.frame).ok_or_else(|| {
            format!(
                "'{}': {}",
                mov.name,
                "Run length encoding required for hitbox frames"
            )
        })?;
        let range = *offset..*offset + first.boxes.len();
        *offset += first.boxes.len();

        run_length_hit_boxes.push((duration, range));
        hit_box_data.extend(first.boxes.iter().map(|hit_box| hit_box.to_hit_box()));
    }
    if let Some(last) = mov.hit_boxes.last() {
        let range = *offset..*offset + last.boxes.len();
        *offset += last.boxes.len();

        run_length_hit_boxes.push((usize::MAX, range));
        hit_box_data.extend(last.boxes.iter().map(|hit_box| hit_box.to_hit_box()));
    } else {
        let range = *offset..*offset;
        run_length_hit_boxes.push((usize::MAX, range));
    }

    Ok(())
}

fn append_hurt_box_data(
    mov: &MoveJson,
    hurt_box_data: &mut Vec<HurtBox>,
    run_length_hurt_boxes: &mut Vec<(usize, Range<usize>)>,
    hurt_boxes_start: &mut Vec<usize>,
    offset: &mut usize,
) -> Result<(), String> {
    hurt_boxes_start.push(run_length_hurt_boxes.len());

    for pair in mov.hurt_boxes.windows(2) {
        let first = &pair[0];
        let second = &pair[1];
        let duration = second.frame.checked_sub(first.frame).ok_or_else(|| {
            format!(
                "'{}': {}",
                mov.name,
                "Run length encoding required for hurtbox frames"
            )
        })?;
        let range = *offset..*offset + first.boxes.len();
        *offset += first.boxes.len();

        run_length_hurt_boxes.push((duration, range));
        hurt_box_data.extend(first.boxes.iter().map(|hit_box| hit_box.to_hurt_box()));
    }
    if let Some(last) = mov.hurt_boxes.last() {
        let range = *offset..*offset + last.boxes.len();
        *offset += last.boxes.len();

        run_length_hurt_boxes.push((usize::MAX, range));
        hurt_box_data.extend(last.boxes.iter().map(|hit_box| hit_box.to_hurt_box()));
    } else {
        let range = *offset..*offset;
        run_length_hurt_boxes.push((usize::MAX, range));
    }

    Ok(())
}

fn append_cancel_options_data(
    mov: &MoveJson,
    map: &HashMap<&str, usize>,
    cancel_options: &mut Vec<Range<usize>>,
    run_length_cancel_options: &mut Vec<usize>,
    offset: &mut usize,
) -> Result<(), String> {
    let range = *offset..*offset + mov.cancel_options.len();
    *offset += mov.cancel_options.len();
    cancel_options.push(range);

    for cancel_option in &mov.cancel_options {
        let index = map
            .get(cancel_option.as_str())
            .ok_or_else(|| format!("Could not find a move named: {cancel_option}"))?;
        run_length_cancel_options.push(*index);
    }
    Ok(())
}

#[derive(Deserialize)]
struct CharacterJson {
    name: String,
    hp: usize,
    moves: Vec<MoveJson>,
    block_stun_state: String,
    ground_hit_state: String,
    launch_hit_state: String,
}

#[derive(Deserialize)]
struct MoveJson {
    name: String,
    input: InputJson,
    hit_boxes: Vec<RunLenJson<HitBoxJson>>,
    hurt_boxes: Vec<RunLenJson<HurtBoxJson>>,
    collision_box: CollisionBoxJson,

    start_behavior: StartBehaviorJson,
    flags: Vec<FlagsJson>,
    end_behavior: EndBehaviorJson,

    cancel_window: CancelWindowJson,
    cancel_options: Vec<String>,

    animation: AnimationJson,
}

#[derive(Deserialize, Clone, Copy)]
#[serde(tag = "type")]
enum StartBehaviorJson {
    None,
    SetVel { x: f32, y: f32 },
    AddFrictionVel { x: f32, y: f32 },
}

impl StartBehaviorJson {
    fn to_start_behavior(self) -> StartBehavior {
        match self {
            StartBehaviorJson::None => StartBehavior::None,
            StartBehaviorJson::SetVel { x, y } => StartBehavior::SetVel { x, y },
            StartBehaviorJson::AddFrictionVel { x, y } => StartBehavior::AddFrictionVel { x, y },
        }
    }
}

#[derive(Deserialize)]
#[serde(tag = "type")]
enum EndBehaviorJson {
    Endless,
    OnFrameXToStateY { x: usize, y: String },
    OnGroundedToStateY { y: String },
    OnStunEndToStateY { y: String },
}

impl EndBehaviorJson {
    fn to_end_behavior(&self, map: &HashMap<&str, usize>) -> Result<EndBehavior, String> {
        Ok(match self {
            EndBehaviorJson::Endless => EndBehavior::Endless,
            EndBehaviorJson::OnFrameXToStateY { x, y } => EndBehavior::OnFrameXToStateY {
                x: *x,
                y: *map.get(y.as_str()).ok_or_else(|| y.clone())?,
            },
            EndBehaviorJson::OnGroundedToStateY { y } => EndBehavior::OnGroundedToStateY {
                y: *map.get(y.as_str()).ok_or_else(|| y.clone())?,
            },
            EndBehaviorJson::OnStunEndToStateY { y } => EndBehavior::OnStunEndToStateY {
                y: *map.get(y.as_str()).ok_or_else(|| y.clone())?,
            },
        })
    }
}

#[derive(Deserialize, Clone, Copy)]
struct CancelWindowJson {
    start: Option<usize>,
    end: Option<usize>,
}

impl CancelWindowJson {
    fn to_range(self) -> Range<usize> {
        self.start.unwrap_or(usize::MAX)..self.end.unwrap_or(usize::MAX)
    }
}

#[derive(Deserialize, Clone, Copy)]
#[serde(tag = "type")]
enum RelativeDirectionJson {
    Any,
    Neutral,
    Up,
    Down,
    Back,
    Forward,
    UpBack,
    DownBack,
    UpForward,
    DownForward,
}

impl RelativeDirectionJson {
    fn to_relative_direction(self) -> RelativeDirection {
        match self {
            RelativeDirectionJson::Any => RelativeDirection::None,
            RelativeDirectionJson::Back => RelativeDirection::Back,
            RelativeDirectionJson::Down => RelativeDirection::Down,
            RelativeDirectionJson::DownBack => RelativeDirection::DownBack,
            RelativeDirectionJson::DownForward => RelativeDirection::DownForward,
            RelativeDirectionJson::Neutral => RelativeDirection::Neutral,
            RelativeDirectionJson::Up => RelativeDirection::Up,
            RelativeDirectionJson::UpBack => RelativeDirection::UpBack,
            RelativeDirectionJson::UpForward => RelativeDirection::UpForward,
            RelativeDirectionJson::Forward => RelativeDirection::Forward,
        }
    }
}

#[derive(Deserialize, Clone, Copy)]
#[serde(tag = "type")]
enum RelativeMotionJson {
    DownDown,
    ForwardForward,
    BackBack,
    QcForward,
    QcBack,
    DpForward,
    DpBack,
}

impl RelativeMotionJson {
    fn to_relative_motion(self) -> RelativeMotion {
        match self {
            RelativeMotionJson::DownDown => RelativeMotion::DownDown,
            RelativeMotionJson::ForwardForward => RelativeMotion::ForwardForward,
            RelativeMotionJson::BackBack => RelativeMotion::BackBack,
            RelativeMotionJson::DpBack => RelativeMotion::DpBack,
            RelativeMotionJson::DpForward => RelativeMotion::DpForward,
            RelativeMotionJson::QcBack => RelativeMotion::QcBack,
            RelativeMotionJson::QcForward => RelativeMotion::QcForward,
        }
    }
}

#[derive(Deserialize, Clone, Copy)]
#[serde(tag = "type")]
enum ButtonJson {
    None,
    L,
    M,
    H,
}

impl ButtonJson {
    fn to_button_flag(self) -> ButtonFlag {
        match self {
            ButtonJson::H => ButtonFlag::H,
            ButtonJson::L => ButtonFlag::L,
            ButtonJson::None => ButtonFlag::NONE,
            ButtonJson::M => ButtonFlag::M,
        }
    }
}

#[derive(Deserialize, Clone, Copy)]
enum InputJson {
    Direction {
        dir: RelativeDirectionJson,
        button: ButtonJson,
    },
    Motion {
        motion: RelativeMotionJson,
        button: ButtonJson,
    },
}

impl InputJson {
    fn to_move_input(self) -> MoveInput {
        match self {
            Self::Direction { dir, button } => MoveInput::new(
                button.to_button_flag(),
                RelativeMotion::NONE,
                dir.to_relative_direction(),
            ),
            Self::Motion { motion, button } => MoveInput::new(
                button.to_button_flag(),
                motion.to_relative_motion(),
                RelativeDirection::None,
            ),
        }
    }
}

#[derive(Deserialize)]
struct RunLenJson<T> {
    frame: usize,
    boxes: Vec<T>,
}

#[derive(Deserialize, Clone, Copy)]
#[serde(tag = "type")]
enum BlockTypeJson {
    Low,
    Mid,
    High,
}

impl BlockTypeJson {
    fn to_block_type(self) -> BlockType {
        match self {
            Self::Low => BlockType::Low,
            Self::Mid => BlockType::Mid,
            Self::High => BlockType::High,
        }
    }
}

#[derive(Deserialize, Clone, Copy)]
struct HitBoxJson {
    rect: RectJson,
    dmg: usize,
    block_stun: u32,
    hit_stun: Option<u32>,
    cancel_window: usize,
    block_type: BlockTypeJson,
}

impl HitBoxJson {
    fn to_hit_box(self) -> HitBox {
        HitBox::new(
            self.rect.to_frect(),
            self.dmg as f32,
            self.block_stun,
            self.hit_stun.unwrap_or(u32::MAX),
            self.cancel_window,
            self.block_type.to_block_type(),
        )
    }
}

#[derive(Deserialize, Clone, Copy)]
struct HurtBoxJson {
    rect: RectJson,
}

impl HurtBoxJson {
    fn to_hurt_box(self) -> HurtBox {
        HurtBox::new(self.rect.to_frect())
    }
}

#[derive(Deserialize, Clone, Copy)]
struct CollisionBoxJson {
    rect: RectJson,
}

impl CollisionBoxJson {
    fn to_collision_box(self) -> CollisionBox {
        CollisionBox::new(self.rect.to_frect())
    }
}

#[derive(Deserialize, Clone, Copy)]
struct RectJson {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

impl RectJson {
    fn to_frect(self) -> FRect {
        FRect::new(self.x - self.w / 2.0, self.y + self.h / 2.0, self.w, self.h)
    }
}

#[derive(Deserialize)]
struct AnimationJson {
    texture_path: String,
    layout: AnimationLayoutJson,
    frames: u32,
    w: u32,
    h: u32,
}

#[derive(Deserialize)]
#[serde(tag = "type")]
pub enum AnimationLayoutJson {
    Horz,
    Vert,
}

impl AnimationLayoutJson {
    fn to_animation_layout(&self) -> AnimationLayout {
        match self {
            AnimationLayoutJson::Horz => AnimationLayout::Horizontal,
            AnimationLayoutJson::Vert => AnimationLayout::Vertical,
        }
    }
}

#[derive(Deserialize)]
#[serde(tag = "type")]
enum FlagsJson {
    Airborne,
    CancelOnWhiff,
    LockSide,
    LowBlock,
    HighBlock,
}

impl FlagsJson {
    fn to_state_json(&self) -> StateFlags {
        match self {
            FlagsJson::Airborne => StateFlags::Airborne,
            FlagsJson::CancelOnWhiff => StateFlags::CancelOnWhiff,
            FlagsJson::LockSide => StateFlags::LockSide,
            FlagsJson::HighBlock => StateFlags::HighBlock,
            FlagsJson::LowBlock => StateFlags::LowBlock,
        }
    }
}
