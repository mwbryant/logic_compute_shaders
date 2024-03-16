use std::{fs::File, path::Path};

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPlugin};
use serde::{Deserialize, Serialize};
use std::io::Write;

use crate::{
    particle_config::{make_random_matrix, ParticleConfig},
    particle_system::RecreateParticles,
};

pub struct ParticleUiPlugin;

#[derive(Debug, Clone, Resource, Reflect, Serialize, Deserialize)]
#[reflect(Resource)]
pub struct UiState {
    pub particle_config: ParticleConfig,
    pub recreate_matrix: bool,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            recreate_matrix: true,
            particle_config: ParticleConfig {
                n: 10_000,
                ..default()
            },
        }
    }
}

impl Plugin for ParticleUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(EguiPlugin)
            .init_resource::<UiState>()
            .add_systems(Startup, configure_visuals_system)
            .add_systems(Update, ui_system);
    }
}

pub fn configure_visuals_system(mut contexts: EguiContexts) {
    contexts.ctx_mut().set_visuals(egui::Visuals {
        window_rounding: 0.0.into(),
        ..default()
    });
}

pub fn ui_system(
    mut egui_contexts: EguiContexts,
    mut ui_state: ResMut<UiState>,
    mut particle_config: ResMut<ParticleConfig>,
    mut event_writer: EventWriter<RecreateParticles>,
) {
    egui::Window::new("Dev Tools").show(egui_contexts.ctx_mut(), |ui| {
        ui.label("Particle Config");
        ui.horizontal(|ui| {
            ui.label("n: ");
            ui.add(
                egui::DragValue::new(&mut ui_state.particle_config.n)
                    .speed(1)
                    .clamp_range(0..=10_000),
            );
        });

        ui.horizontal(|ui| {
            ui.label("dt: ");
            ui.add(
                egui::DragValue::new(&mut ui_state.particle_config.dt)
                    .speed(0.0001)
                    .clamp_range(0.0..=1.0),
            );
        });
        ui.horizontal(|ui| {
            ui.label("friction_half_life: ");
            ui.add(
                egui::DragValue::new(&mut ui_state.particle_config.friction_half_life)
                    .speed(0.0001)
                    .clamp_range(0.0..=1.0),
            );
        });
        ui.horizontal(|ui| {
            ui.label("r_max: ");
            ui.add(
                egui::DragValue::new(&mut ui_state.particle_config.r_max)
                    .speed(1.0)
                    .clamp_range(0.0..=500.0),
            );
        });
        ui.horizontal(|ui| {
            ui.label("m: ");
            ui.add(
                egui::DragValue::new(&mut ui_state.particle_config.m)
                    .speed(1)
                    .clamp_range(0..=32),
            );
            ui_state.particle_config.attraction_matrix =
                make_random_matrix(ui_state.particle_config.m);
        });
        ui.horizontal(|ui| {
            ui.label("force_factor: ");
            ui.add(
                egui::DragValue::new(&mut ui_state.particle_config.force_factor)
                    .speed(1.0)
                    .clamp_range(0.0..=500.0),
            );
        });
        
        ui.horizontal(|ui| {
            ui.label("recreate_matrix: ");
            ui.add(egui::Checkbox::new(
                &mut ui_state.recreate_matrix,
                "Recreate Matrix",
            ));
        });

        ui.label(format!(
            "friction_factor: {}",
            0.5f32.powf(ui_state.particle_config.dt / ui_state.particle_config.friction_half_life)
        ));

        if ui.button("Run").clicked() {
            particle_config.n = ui_state.particle_config.n;
            particle_config.dt = ui_state.particle_config.dt;
            particle_config.friction_half_life = ui_state.particle_config.friction_half_life;
            particle_config.r_max = ui_state.particle_config.r_max;
            particle_config.m = ui_state.particle_config.m;
            particle_config.force_factor = ui_state.particle_config.force_factor;
            particle_config.friction_factor =
                0.5f32.powf(particle_config.dt / particle_config.friction_half_life);
            if ui_state.recreate_matrix {
                particle_config.attraction_matrix = make_random_matrix(particle_config.m);
            }
            event_writer.send(RecreateParticles);
        }

        //     if ui.button("Run").clicked() {
        //         //*particle_config = ParticleConfig::default();
        //       //  particle_config.recreate = true;
        //         // commands.run_once(create_particles);
        //     }

        //     // ui.horizontal(|ui| {
        //     //     ui.label("Save/Load Config: ");
        //     //     ui.text_edit_singleline(&mut particle_config.save_load_name);
        //     // });

        //     // egui::ComboBox::from_label("Choose a file")
        //     //     .selected_text(particle_config.save_load_name.clone())
        //     //     .show_ui(ui, |ui| {
        //     //         let file_names = std::fs::read_dir("assets/particle_configs")
        //     //             .unwrap()
        //     //             .map(|entry| {
        //     //                 let path = entry.unwrap().path();
        //     //                 path.file_stem()
        //     //                     .and_then(|stem| stem.to_str())
        //     //                     .map(|s| s.to_owned())
        //     //                     .unwrap_or_default()
        //     //             })
        //     //             .collect::<Vec<String>>();
        //     //         for file_name in file_names.iter() {
        //     //             ui.selectable_value(
        //     //                 &mut particle_config.save_load_name,
        //     //                 file_name.to_string(),
        //     //                 file_name,
        //     //             );
        //     //         }
        //     //     });

        //     // if ui.button("Save").clicked() {
        //     //     save_particle_config(
        //     //         particle_config.clone(),
        //     //         particle_config.save_load_name.to_string(),
        //     //     );
        //     // }

        //     // if ui.button("Load").clicked() {
        //     //     *particle_config = load_particle_config(particle_config.save_load_name.to_string());
        //     // }
    });
}

fn save_particle_config(particle_config: ParticleConfig, name: String) {
    let serialized = serde_json::to_string(&particle_config).unwrap();

    let folder_name = "assets/particle_configs";
    let file_name = format!("{}.json", name);
    let path = Path::new(folder_name).join(file_name);

    std::fs::create_dir_all(folder_name).unwrap();

    let mut file = File::create(path).unwrap();
    file.write_all(serialized.as_bytes()).unwrap();
}

fn load_particle_config(name: String) -> ParticleConfig {
    let folder_name = "assets/particle_configs";
    let file_name = format!("{}.json", name);
    let path = Path::new(folder_name).join(file_name);

    let file = File::open(path).unwrap();
    let reader = std::io::BufReader::new(file);

    let particle_config: ParticleConfig = serde_json::from_reader(reader).unwrap();
    particle_config
}
