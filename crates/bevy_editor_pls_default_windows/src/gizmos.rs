use bevy::{
    ecs::query::QueryFilter,
    prelude::*,
    render::{camera::CameraProjection, view::RenderLayers},
};

use bevy_editor_pls_core::editor_window::{EditorWindow, EditorWindowContext};
use bevy_inspector_egui::egui;
use transform_gizmo_egui::{GizmoExt, GizmoMode};

use crate::{
    cameras::{ActiveEditorCamera, EditorCamera, EDITOR_RENDER_LAYER},
    hierarchy::HierarchyWindow,
};

pub struct GizmoState {
    pub camera_gizmo_active: bool,
    pub gizmo_mode: transform_gizmo_egui::EnumSet<transform_gizmo_egui::GizmoMode>,
    pub gizmo: transform_gizmo_egui::Gizmo,
}

impl Default for GizmoState {
    fn default() -> Self {
        Self {
            camera_gizmo_active: true,
            gizmo: transform_gizmo_egui::Gizmo::default(),
            gizmo_mode: transform_gizmo_egui::EnumSet::only(GizmoMode::Translate),
        }
    }
}

pub struct GizmoWindow;

impl EditorWindow for GizmoWindow {
    type State = GizmoState;

    const NAME: &'static str = "Gizmos";

    fn ui(_world: &mut World, _cx: EditorWindowContext, ui: &mut egui::Ui) {
        ui.label("Gizmos can currently not be configured");
    }

    fn viewport_toolbar_ui(world: &mut World, mut cx: EditorWindowContext, ui: &mut egui::Ui) {
        let (camera_gizmo_active, gizmo_mode) = {
            let GizmoState {
                camera_gizmo_active,
                gizmo_mode,
                ..
            } = cx.state::<GizmoWindow>().unwrap();
            (*camera_gizmo_active, *gizmo_mode)
        };

        if camera_gizmo_active {
            if let Some(new_config) = collect_gizmo_config(ui, world, gizmo_mode) {
                cx.state_mut::<GizmoWindow>()
                    .unwrap()
                    .gizmo
                    .update_config(new_config);
            }
            let selected_entities = cx
                .state::<HierarchyWindow>()
                .unwrap()
                .selected.iter().collect::<Vec<_>>();

            let gizmo_state_mut = cx.state_mut::<GizmoWindow>().unwrap();
            draw_gizmo(ui, world, &selected_entities, &mut gizmo_state_mut.gizmo);
        }
    }

    fn app_setup(app: &mut App) {
        let mut materials = app.world.resource_mut::<Assets<StandardMaterial>>();
        let material_light = materials.add(StandardMaterial {
            base_color: Color::rgba_u8(222, 208, 103, 255),
            unlit: true,
            fog_enabled: false,
            alpha_mode: AlphaMode::Add,
            ..default()
        });
        let material_camera = materials.add(StandardMaterial {
            base_color: Color::rgb(1.0, 1.0, 1.0),
            unlit: true,
            fog_enabled: false,
            alpha_mode: AlphaMode::Multiply,
            ..default()
        });

        let mut meshes = app.world.resource_mut::<Assets<Mesh>>();
        let sphere = meshes.add(Sphere { radius: 0.3 });

        app.world.insert_resource(GizmoMarkerConfig {
            point_light_mesh: sphere.clone(),
            point_light_material: material_light.clone(),
            directional_light_mesh: sphere.clone(),
            directional_light_material: material_light,
            camera_mesh: sphere,
            camera_material: material_camera,
        });

        app.add_systems(PostUpdate, add_gizmo_markers);
    }
}

#[derive(Resource)]
struct GizmoMarkerConfig {
    point_light_mesh: Handle<Mesh>,
    point_light_material: Handle<StandardMaterial>,
    directional_light_mesh: Handle<Mesh>,
    directional_light_material: Handle<StandardMaterial>,
    camera_mesh: Handle<Mesh>,
    camera_material: Handle<StandardMaterial>,
}

#[derive(Component)]
struct HasGizmoMarker;

type GizmoMarkerQuery<'w, 's, T, F = ()> =
    Query<'w, 's, Entity, (With<T>, Without<HasGizmoMarker>, F)>;

fn add_gizmo_markers(
    mut commands: Commands,
    gizmo_marker_meshes: Res<GizmoMarkerConfig>,

    point_lights: GizmoMarkerQuery<PointLight>,
    directional_lights: GizmoMarkerQuery<DirectionalLight>,
    cameras: GizmoMarkerQuery<Camera, Without<EditorCamera>>,
) {
    fn add<T: Component, F: QueryFilter, B: Bundle>(
        commands: &mut Commands,
        query: GizmoMarkerQuery<T, F>,
        name: &'static str,
        f: impl Fn() -> B,
    ) {
        let render_layers = RenderLayers::layer(EDITOR_RENDER_LAYER);
        for entity in &query {
            commands
                .entity(entity)
                .insert(HasGizmoMarker)
                .with_children(|commands| {
                    commands.spawn((f(), render_layers, Name::new(name)));
                });
        }
    }

    add(&mut commands, point_lights, "PointLight Gizmo", || {
        PbrBundle {
            mesh: gizmo_marker_meshes.point_light_mesh.clone_weak(),
            material: gizmo_marker_meshes.point_light_material.clone_weak(),
            ..default()
        }
    });
    add(
        &mut commands,
        directional_lights,
        "DirectionalLight Gizmo",
        || PbrBundle {
            mesh: gizmo_marker_meshes.directional_light_mesh.clone_weak(),
            material: gizmo_marker_meshes.directional_light_material.clone_weak(),
            ..default()
        },
    );

    let render_layers = RenderLayers::layer(EDITOR_RENDER_LAYER);
    for entity in &cameras {
        commands
            .entity(entity)
            .insert((
                HasGizmoMarker,
                Visibility::Visible,
                InheritedVisibility::VISIBLE,
                ViewVisibility::default(),
            ))
            .with_children(|commands| {
                commands.spawn((
                    PbrBundle {
                        mesh: gizmo_marker_meshes.camera_mesh.clone_weak(),
                        material: gizmo_marker_meshes.camera_material.clone_weak(),
                        ..default()
                    },
                    render_layers,
                    Name::new("Camera Gizmo"),
                ));
            });
    }
}
fn convert_array_f32_to_f64<const N: usize>(a: &[f32; N]) -> [f64; N] {
    let mut result = [0.0; N];
    for i in 0..N {
        result[i] = a[i] as f64;
    }
    result
}
fn collect_gizmo_config(
    ui: &mut egui::Ui,
    world: &mut World,
    gizmo_mode: transform_gizmo_egui::EnumSet<transform_gizmo_egui::GizmoMode>,
) -> Option<transform_gizmo_egui::GizmoConfig> {
    let (cam_transform, projection) = world
        .query_filtered::<(&GlobalTransform, &Projection), With<ActiveEditorCamera>>()
        .get_single(world)
        .ok()?;
    let view_matrix = Mat4::from(cam_transform.affine().inverse());
    let projection_matrix = projection.get_projection_matrix();

    let transform_view_matrix = transform_gizmo_egui::math::DMat4::from_cols_array(
        &convert_array_f32_to_f64(&view_matrix.to_cols_array()),
    );
    let transform_projection_matrix = transform_gizmo_egui::math::DMat4::from_cols_array(
        &convert_array_f32_to_f64(&projection_matrix.to_cols_array()),
    );
    Some(transform_gizmo_egui::GizmoConfig {
        modes: gizmo_mode,
        viewport: ui.clip_rect(),
        orientation: transform_gizmo_egui::GizmoOrientation::Local,
        view_matrix: transform_view_matrix.into(),
        projection_matrix: transform_projection_matrix.into(),
        ..Default::default()
    })
}
fn draw_gizmo(
    ui: &mut egui::Ui,
    world: &mut World,
    selected_entities: &[Entity],
    gizmo: &mut transform_gizmo_egui::Gizmo,
) {
    let all_transform_and_entity = selected_entities.iter().filter_map(|selected| {
        let Some(global_transform) = world.get::<GlobalTransform>(*selected) else {
            return None;
        };
    
        let (scale, rotation, translation) = global_transform.to_scale_rotation_translation();
    
        let gizmo_transform = transform_gizmo_egui::math::Transform::from_scale_rotation_translation(
            transform_gizmo_egui::mint::Vector3::from_slice(&convert_array_f32_to_f64(
                &scale.to_array(),
            )),
            transform_gizmo_egui::mint::Quaternion::from(convert_array_f32_to_f64(
                &rotation.to_array(),
            )),
            transform_gizmo_egui::mint::Vector3::from_slice(&convert_array_f32_to_f64(
                &translation.to_array(),
            )),
        );
        Some((gizmo_transform, *selected))
    }).collect::<Vec<_>>();

    let all_transform = all_transform_and_entity.iter().map(|(transform,_)|transform.clone()).collect::<Vec<_>>();
    let Some((_, transforms)) = gizmo.interact(ui,& all_transform) else {
        return;
    };

    for ((_,entity), result) in all_transform_and_entity.iter().zip(transforms.iter()) {
        let global_affine = world.get::<GlobalTransform>(*entity).unwrap().affine();
        let mut transform = world.get_mut::<Transform>(*entity).unwrap();
        let parent_affine = global_affine * transform.compute_affine().inverse();
        let inverse_parent_transform = GlobalTransform::from(parent_affine.inverse());
        let transform_gizmo_egui::math::Transform {
            scale,
            translation,
            rotation,
        } = result;
        let global_transform = Transform {
            scale: Vec3::new(scale.x as f32, scale.y as f32, scale.z as f32),
            translation: Vec3::new(
                translation.x as f32,
                translation.y as f32,
                translation.z as f32,
            ),
            rotation: Quat::from_xyzw(
                rotation.v.x as f32,
                rotation.v.y as f32,
                rotation.v.z as f32,
                rotation.s as f32,
            ),
        };
        *transform = (inverse_parent_transform * global_transform).into();
    }

}
