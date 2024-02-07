use std::collections::HashMap;

use bevy::input::keyboard::KeyboardInput;
use bevy::input::ButtonState;
use bevy::utils::info;
use bevy::window::WindowResolution;
use bevy_egui::egui::ahash::HashMapExt;
use rand::seq::{ SliceRandom, IteratorRandom };

use bevy::{prelude::*, time::Stopwatch};
use bevy_egui::{ EguiContexts, EguiPlugin };
use bevy_inspector_egui::prelude::*;
use bevy_inspector_egui::quick::ResourceInspectorPlugin;
use rand::Rng;

const HITBOX_RAD: f32 = 32.;

#[derive(Component)]
struct Ingredient;

#[derive(Component)]
struct Food;

#[derive(Component)]
struct Processing;

#[derive(Component)]
struct Active(Entity);

#[derive(Component)]
struct Tex(Handle<Image>);

#[derive(Resource, Default)]
struct Ingredients(HashMap<Id, Entity>);

#[derive(Event)]
struct ActiveFoodUpdated;

#[derive(Resource, Default)]
struct FoodsCount(u32);

struct IngredientProcessing {
    ingredient: Id,
    processing: Id
}

#[derive(Component)]
struct FoodIngredients(Vec<IngredientProcessing>);

#[derive(Component, Clone, PartialEq, Eq, Hash)]
struct Id(String);

#[derive(Bundle)]
struct FoodBundle {
    id: Id,
    ingredients: FoodIngredients,
    tex: Tex,
    marker: Food
}

#[derive(Component, Default)]
struct IngredientTodo(Vec<usize>);

#[derive(Bundle)]
struct IngredientBundle {
    id: Id,
    tex: Tex,
    marker: Ingredient
}

#[derive(Resource)]
struct KeyMapping(HashMap<KeyCode, Entity>);

#[derive(Bundle)]
struct ProcessingBundle {
    id: Id,
    tex: Tex,
    marker: Processing
}

#[derive(Event)]
struct Process(Entity, Vec2);

#[derive(Reflect, Resource, Default, InspectorOptions)]
#[reflect(Resource, InspectorOptions)]
struct ThrowConfig {
    #[inspector(min = -25.0, max = 25.0)]
    time: f32,
    #[inspector(min = -500.0, max = 500.0)]
    height: f32,
    #[inspector(min = -500.0, max = 500.0)]
    drift: f32
}

#[derive(Resource)]
struct IngredientSpawnTimer(Timer);

#[derive(Resource)]
struct FoodSpawnTimer(Timer);

#[derive(Resource)]
struct DespawnTimer(Timer);

#[derive(Component)]
struct Throw {
    t: Stopwatch,
    g: f32,
    v: f32,
    drift: f32,
    spawn_x: f32,
    spawn_y: f32
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins
            .set(ImagePlugin::default_nearest())
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "yo".into(),
                    ..Default::default()
                }),
                ..Default::default()
            }))
        .add_plugins(EguiPlugin)
        .init_resource::<Ingredients>()
        .init_resource::<FoodsCount>()
        .register_type::<ThrowConfig>()
        .add_event::<ActiveFoodUpdated>()
        .add_event::<Process>()
        .add_plugins(ResourceInspectorPlugin::<ThrowConfig>::default())
        .add_systems(Startup, setup)
        .add_systems(Update, (spawn_ingredients, move_ingredients, despawn_ingredients, spawn_foods, draw_active_foods, keypress, process))
        .run();
}

#[derive(Event)]
struct ProcessIngredient {
    active: Entity,
    ingredient: Entity,
    process: Entity
}

#[derive(Event)]
struct RecipeComplete(Entity);

fn process(mut evts: EventReader<Process>, mut next: EventWriter<ProcessIngredient>, active: Query<(Entity, &Active, &Transform), With<Ingredient>>) {
    for process in evts.read() {
        let Some((e, a, _, dist)) = active
            .iter()
            .map(|(e, a, t)| (e, a, t, t.translation.xy().distance(process.1)))
            .min_by_key(|(_, _, _, d)| *d as usize)
        else { return };
        info!("found closest entity: {:?}, {} away from cursor", e, dist);
        if dist <= HITBOX_RAD {
            //cmd.entity(e).despawn();
            next.send(ProcessIngredient{ active: e, ingredient: a.0, process: process.0 });
        }
    }
}

fn process_ingredient(
    mut evts: EventReader<ProcessIngredient>,
    mut changed: EventWriter<ActiveFoodUpdated>,
    mut completed: EventWriter<RecipeComplete>,
    foods_a: Query<(Entity, &mut IngredientTodo, &Active), With<Food>>,
    foods: Query<&FoodIngredients, With<Food>>,
    ids: Query<&Id>) {
    for evt in evts.read() {
        let tool_id = ids.get(evt.process).unwrap();
        let ingredient_id = ids.get(evt.ingredient).unwrap();

        let x = foods_a
            .iter()
            .find_map(|(e, todo, a)| {
                let food_ingredients = foods.get(a.0).unwrap();
                food_ingredients.0
                    .iter()
                    .enumerate()
                    .find(|(i, process)| todo.0.contains(&i) && process.ingredient == *ingredient_id && process.processing == *tool_id)
            });

        for (e, todo, a) in foods_a.iter() {
            let food_ingredients = foods.get(e).unwrap();
            let Some((i, process)) = food_ingredients.0
                .iter()
                .enumerate()
                .find(|(_, process)| process.ingredient == *ingredient_id && process.processing == *tool_id)
            else { continue };

            if todo.0.contains(&i) { continue };
        }
    }
}

fn keypress(mut keyevt: EventReader<KeyboardInput>, keymap: Res<KeyMapping>, mut process: EventWriter<Process>, w: Query<&Window>, cameraq: Query<(&Camera, &GlobalTransform)>) {
    let Some(cursor) = w.single().cursor_position() else { return };
    let (cam, cam_transform) = cameraq.single();
    let coords = cam.viewport_to_world_2d(cam_transform, cursor).unwrap();
    for ev in keyevt.read() {
        if ev.state != ButtonState::Pressed { continue };
        let Some(key) = ev.key_code else { continue };
        let Some(tool) = keymap.0.get(&key) else { continue };
        process.send(Process(*tool, coords));
    }
}

fn draw_active_foods(mut evts: EventReader<ActiveFoodUpdated>, mut active: Query<(&Active, &mut Transform, &mut Visibility), With<Food>>, tex: Query<&Tex, With<Food>>, w: Query<&Window>, assets: Res<Assets<Image>>) {
    if evts.is_empty() { return; }
    evts.clear();
    
    let res = &w.single().resolution;
    // redraw active foods
    for (i, (a, mut transform, mut vis)) in active.iter_mut().enumerate() {
        let tex = tex.get(a.0).unwrap();
        let Some(asset) = assets.get(&tex.0) else { continue; };

        transform.translation.y = -res.height() / 2. + 50.;
        transform.translation.x = -res.width() / 2. + 50. + ((i + 1) * asset.size().x as usize) as f32;
        //transform.translation.z = 1000.;
        *vis = Visibility::Visible;
    }
}

fn add_ingredient(cmd: &mut Commands, name: Id, assets: &AssetServer, ingredients: &mut Ingredients) -> Id {
    let handle: Handle<Image> = assets.load(name.0.clone() + ".png");
    ingredients.0.insert(name.clone(), cmd.spawn(IngredientBundle {
        id: name.clone(),
        tex: Tex(handle),
        marker: Ingredient
    }).id());

    name
}

fn add_food(cmd: &mut Commands, name: Id, assets: &AssetServer, ingredients: FoodIngredients) {
    let handle: Handle<Image> = assets.load(name.0.clone() + ".png");
    cmd.spawn(FoodBundle {
        id: name,
        tex: Tex(handle),
        ingredients,
        marker: Food
    });
}

fn add_processing(cmd: &mut Commands, name: Id, assets: &AssetServer, count: &mut u32, res: &WindowResolution) -> (Entity, Id) {
    let handle: Handle<Image> = assets.load(name.0.clone() + ".png");
    let e = cmd.spawn((
        ProcessingBundle {
            id: name.clone(),
            tex: Tex(handle.clone()),
            marker: Processing,
        },
        SpriteBundle {
            texture: handle.clone(),
            transform: Transform::from_xyz(res.width() / 2. + 50. , res.height() / 2. + 50. * (*count as f32), 0.),
            ..default()
        }
    )).id();

    *count += 1;
    (e, name)
}

fn setup(mut cmd: Commands, assets: Res<AssetServer>, mut ingredients: ResMut<Ingredients>, w: Query<&Window>) {
    let res = &w.single().resolution;
    cmd.spawn(Camera2dBundle::default());

    cmd.insert_resource(IngredientSpawnTimer(Timer::from_seconds(3., TimerMode::Repeating)));
    cmd.insert_resource(DespawnTimer(Timer::from_seconds(1., TimerMode::Repeating)));
    cmd.insert_resource(FoodSpawnTimer(Timer::from_seconds(10., TimerMode::Repeating)));

    let tomato = add_ingredient(&mut cmd, Id("tomato".to_string()), &assets, &mut ingredients);
    let egg = add_ingredient(&mut cmd, Id("egg".to_string()), &assets, &mut ingredients);

    let mut count = 0;
    let (pane, pan) = add_processing(&mut cmd, Id("pan".to_string()), &assets, &mut count, res);

    let mut keymap = KeyMapping(HashMap::new());
    keymap.0.insert(KeyCode::Key1, pane);

    cmd.insert_resource(keymap);

    add_food(&mut cmd,
        Id("fried_egg".to_string()),
        &assets,
        FoodIngredients(
            vec![IngredientProcessing { ingredient: egg.clone(), processing: pan }]
        ));

    cmd.insert_resource(ThrowConfig { time: 1., height: 100., drift: 100. });
}

fn spawn_foods(mut cmd: Commands, foods: Query<(Entity, &Tex, &FoodIngredients), With<Food>>, time: Res<Time>, mut timer: ResMut<FoodSpawnTimer>, mut evt: EventWriter<ActiveFoodUpdated>) {
    if ! timer.0.tick(time.delta()).finished() { return; }

    let (e, tex, ingredients) = foods.iter().choose(&mut rand::thread_rng()).unwrap();
    cmd.spawn((
        Active(e),
        Food{},
        IngredientTodo::default(),
        SpriteBundle {
            texture: tex.0.clone(),
            visibility: Visibility::Hidden,
            ..default()
        }
    ));

    info!("new active food event sent!");
    evt.send(ActiveFoodUpdated {});
}

fn spawn_ingredients(mut cmd: Commands, time: Res<Time>, mut timer: ResMut<IngredientSpawnTimer>, cfg: Res<ThrowConfig>, foods: Query<&Active, With<Food>>, food_ingredients: Query<&FoodIngredients>, ingredients: Res<Ingredients>, tex: Query<&Tex, With<Ingredient>>, w: Query<&Window>) {
    if ! timer.0.tick(time.delta()).finished() { return; }

    let Some(food) = foods.iter().choose(&mut rand::thread_rng()) else { return };
    let ingredient_id = &food_ingredients.get(food.0).unwrap().0.choose(&mut rand::thread_rng()).unwrap().ingredient;
    let ingredient = ingredients.0.get(ingredient_id).unwrap();

    let g = cfg.height / 2.0 * cfg.time.powi(2);
    let v = f32::sqrt(2. * cfg.height * g);
    let drift = f32::sqrt(2. * cfg.drift.abs() * g) * cfg.drift.min(1.).max(-1.);

    let res = &w.single().resolution;
    let mut rng = rand::thread_rng();

    let spawn_x = rng.gen_range(- res.width() / 2. .. res.width() / 2.);
    let spawn_y = - res.height() / 2.;

    cmd.spawn((
        Active(*ingredient),
        Ingredient {},
        SpriteBundle {
            texture: tex.get(*ingredient).unwrap().0.clone(),
            transform: Transform::default(),
            ..default()
        },
        Throw {
            spawn_x,
            spawn_y,
            drift,
            t: Stopwatch::default(),
            v,
            g
        }
    ));
}

fn despawn_ingredients(mut cmd: Commands, q: Query<(Entity, &Transform), With<Throw>>, time: Res<Time>, mut timer: ResMut<DespawnTimer>, w: Query<&Window>) {
    if ! timer.0.tick(time.delta()).finished() { return; }
    let res = &w.single().resolution;
    for (e, _) in q.iter().filter(|(_, t)| t.translation.y < - res.height() / 2.) {
        cmd.entity(e).despawn()
    }
}

fn move_ingredients(mut q: Query<(&mut Transform, &mut Throw)>, time: Res<Time>) {
    for (mut transform, mut throw) in q.iter_mut() {
        throw.t.tick(time.delta());
        let t = throw.t.elapsed_secs();
        transform.translation.y = -throw.g * t.powi(2) + throw.v * t + throw.spawn_y;
        transform.translation.x = throw.drift * t + throw.spawn_x;
    }
}
