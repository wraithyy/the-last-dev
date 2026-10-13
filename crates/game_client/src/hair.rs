//! The Senior Engineer's hair thins with HP: the player model's hair is split
//! into `Hair_1` (crown spot) .. `Hair_5` (last remaining sides), and every part
//! up to the current stage is hidden. HP coming back restores it. Other players
//! in an online room wear the same model and thin the same way.

use crate::combat::{SimReady, SimUnit, SimView};
use crate::hud::LogLineMsg;
use crate::net::RemotePlayer;
use crate::player::Player;
use bevy::gltf::GltfMaterialName;
use bevy::prelude::*;

/// The local player and the other players of an online room wear the same model.
type Engineer = Or<(With<Player>, With<RemotePlayer>)>;

/// Hair mesh entities of the player model with their stage (1..=5).
#[derive(Component)]
struct HairParts(Vec<(Entity, u8)>);

pub struct HairPlugin;

impl Plugin for HairPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (find_hair, thin_hair).chain().in_set(SimReady));
    }
}

/// 0 = full hair .. 5 = completely bald.
pub fn hair_stage(hp: u32, max_hp: u32) -> u8 {
    let pct = 100.0 * hp as f32 / max_hp.max(1) as f32;
    match pct {
        p if p > 90.0 => 0,
        p if p > 70.0 => 1,
        p if p > 50.0 => 2,
        p if p > 30.0 => 3,
        p if p > 12.0 => 4,
        _ => 5,
    }
}

/// The glTF scene spawns asynchronously, so keep looking until the parts exist.
fn find_hair(
    mut commands: Commands,
    engineers: Query<Entity, (Engineer, Without<HairParts>)>,
    children: Query<&Children>,
    names: Query<&GltfMaterialName>,
) {
    for engineer in &engineers {
        let parts: Vec<(Entity, u8)> = children
            .iter_descendants(engineer)
            .filter_map(|e| {
                let stage = names.get(e).ok()?.0.strip_prefix("Hair_")?.parse().ok()?;
                Some((e, stage))
            })
            .collect();
        if !parts.is_empty() {
            commands.entity(engineer).insert(HairParts(parts));
        }
    }
}

fn thin_hair(
    sim: SimView,
    engineers: Query<(&SimUnit, &HairParts, Has<Player>)>,
    mut vis: Query<&mut Visibility>,
    mut log: MessageWriter<LogLineMsg>,
    mut joked: Local<bool>,
) {
    for (su, parts, is_player) in &engineers {
        let Some(u) = sim.unit(su.0) else { continue };
        let stage = hair_stage(u.hp, u.max_hp);
        for (e, k) in &parts.0 {
            if let Ok(mut v) = vis.get_mut(*e) {
                v.set_if_neq(if *k <= stage {
                    Visibility::Hidden
                } else {
                    Visibility::Inherited
                });
            }
        }
        if is_player && stage == 5 && u.alive && u.in_combat && !*joked {
            *joked = true;
            log.write(LogLineMsg(
                "You have lost all your hair. The AI did not notice.".into(),
                Color::srgb(0.5, 0.5, 0.56),
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::hair_stage;

    #[test]
    fn hair_thins_in_five_steps() {
        assert_eq!(hair_stage(100, 100), 0);
        assert_eq!(hair_stage(91, 100), 0);
        assert_eq!(hair_stage(90, 100), 1);
        assert_eq!(hair_stage(60, 100), 2);
        assert_eq!(hair_stage(40, 100), 3);
        assert_eq!(hair_stage(13, 100), 4);
        assert_eq!(hair_stage(12, 100), 5);
        assert_eq!(hair_stage(0, 0), 5);
    }
}
