mod ui;
use ui::MenuPlugin;
use std::{collections::HashMap, time::Duration};

use bevy::input::keyboard::KeyboardInput;
use bevy::input::ButtonState;
use bevy::window::WindowResolution;
use rand::seq::{ SliceRandom, IteratorRandom };

use bevy::{prelude::*, time::Stopwatch};
use rand::Rng;

const HITBOX_RAD: f32 = 50.;
const FOOD_SPAWN: f32 = 10.;
const INGREDIENT_SPAWN: f32 = 3.;

#[derive(Resource, Default)]
struct Score(usize);

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

#[derive(Resource, Default)]
struct FoodsCount(u32);

#[derive(Clone)]
struct IngredientProcessing {
    ingredient: Id,
    processing: Id
}

#[derive(Component, Clone)]
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

#[derive(Reflect, Resource, Default)]
#[reflect(Resource)]
struct ThrowConfig {
    time: f32,
    height: f32,
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

#[derive(Clone, Copy, Default, Eq, PartialEq, Debug, Hash, States)]
enum GameState {
    #[default]
    MainMenu,
    Game,
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins
            .set(ImagePlugin::default_nearest())
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "yo".into(),
                    resolution: (720., 480.).into(),
                    resizable: false,
                    ..Default::default()
                }),
                ..Default::default()
            }))
        .add_plugins(MenuPlugin)
        .insert_resource(ClearColor(Color::rgb(255. / 255., 105. / 255., 180. / 255.)))
        .add_state::<GameState>()
        .init_resource::<Ingredients>()
        .init_resource::<FoodsCount>()
        .init_resource::<Score>()
        .register_type::<ThrowConfig>()
        .add_event::<Process>()
        .add_event::<ProcessIngredient>()
        .add_event::<RecipeComplete>()
        .add_systems(Startup, setup)
        .add_systems(OnEnter(GameState::Game), reset_score)
        .add_systems(Update, (
            spawn_ingredients,
            move_ingredients,
            despawn_ingredients,
            spawn_foods,
            draw_active_foods,
            keypress,
            process,
            process_ingredient,
            draw_processing,
            loose,
            count_score)
            .run_if(in_state(GameState::Game)))
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

fn reset_score(mut score: ResMut<Score>) {
    score.0 = 0;
}

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
    mut cmd: Commands,
    mut evts: EventReader<ProcessIngredient>,
    mut completed: EventWriter<RecipeComplete>,
    mut foods_a: Query<(Entity, &mut FoodIngredients, &Active), With<Food>>,
    ids: Query<&Id>) {
    for evt in evts.read() {
        let tool_id = ids.get(evt.process).unwrap();
        let ingredient_id = ids.get(evt.ingredient).unwrap();

        let Some((foode, ii)) = foods_a
            .iter()
            .find_map(|(e, ingredients, a)| {
                ingredients.0
                    .iter()
                    .enumerate()
                    .find(|(i, process)| process.ingredient == *ingredient_id && process.processing == *tool_id)
                    .map(|(i, _)| (e, i))
            }) else {
                cmd.entity(evt.active);
                continue;
            };

        let (_, mut ingredients, _) = foods_a.get_mut(foode).unwrap();
        ingredients.0.remove(ii);
        cmd.entity(evt.active).despawn();
        if ingredients.0.len() == 0 {
            cmd.entity(foode).despawn();
            completed.send(RecipeComplete(foode));
        }
    }
}

fn count_score(mut complete: EventReader<RecipeComplete>, mut score: ResMut<Score>) {
    for _ in complete.read() {
        score.0 += 1;
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

fn draw_active_foods(mut active: Query<(&Active, &mut Transform, &mut Visibility), With<Food>>, tex: Query<&Tex, With<Food>>, w: Query<&Window>, assets: Res<Assets<Image>>) {
    let res = &w.single().resolution;
    // redraw active foods
    for (i, (a, mut transform, mut vis)) in active.iter_mut().enumerate() {
        let tex = tex.get(a.0).unwrap();
        let Some(asset) = assets.get(&tex.0) else { continue; };

        transform.translation.y = -res.height() / 2. + 50.;
        transform.translation.x = -res.width() / 2. + ((i + 1) * 75) as f32 - 15.;
        //transform.translation.z = 1000.;
        *vis = Visibility::Visible;
    }
}

fn draw_processing(mut tools: Query<(Entity, &mut Transform), With<Processing>>, tex: Query<&Tex>, w: Query<&Window>, assets: Res<Assets<Image>>) {
    let res = &w.single().resolution;
    // redraw active foods
    for (i, (e, mut transform)) in tools.iter_mut().enumerate() {
        let tex = tex.get(e).unwrap();
        let Some(asset) = assets.get(&tex.0) else { continue; };

        transform.translation.y = res.height() / 2. - ((i + 1) * (asset.size().x as usize + 15)) as f32;
        transform.translation.x = -res.width() / 2. + 50. ;
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

fn add_processing(cmd: &mut Commands, name: Id, assets: &AssetServer, res: &WindowResolution) -> (Entity, Id) {
    let handle: Handle<Image> = assets.load(name.0.clone() + ".png");
    let e = cmd.spawn((
        ProcessingBundle {
            id: name.clone(),
            tex: Tex(handle.clone()),
            marker: Processing,
        },
        SpriteBundle {
            texture: handle.clone(),
            transform: Transform::default(),
            ..default()
        }
    )).id();

    (e, name)
}

fn setup(mut cmd: Commands, assets: Res<AssetServer>, mut ingredients: ResMut<Ingredients>, w: Query<&Window>) {
    let res = &w.single().resolution;
    cmd.spawn(Camera2dBundle::default());

    cmd.insert_resource(IngredientSpawnTimer(Timer::from_seconds(INGREDIENT_SPAWN, TimerMode::Repeating)));
    cmd.insert_resource(DespawnTimer(Timer::from_seconds(1., TimerMode::Repeating)));
    cmd.insert_resource(FoodSpawnTimer(Timer::from_seconds(FOOD_SPAWN, TimerMode::Repeating)));

    let tomato = add_ingredient(&mut cmd, Id("tomato".to_string()), &assets, &mut ingredients);
    let egg = add_ingredient(&mut cmd, Id("egg".to_string()), &assets, &mut ingredients);
    let bread = add_ingredient(&mut cmd, Id("bread".to_string()), &assets, &mut ingredients);
    let carrot = add_ingredient(&mut cmd, Id("carrot".to_string()), &assets, &mut ingredients);
    let fish = add_ingredient(&mut cmd, Id("fish".to_string()), &assets, &mut ingredients);
    let garlic = add_ingredient(&mut cmd, Id("garlic".to_string()), &assets, &mut ingredients);
    let orange = add_ingredient(&mut cmd, Id("orange".to_string()), &assets, &mut ingredients);
    let nori = add_ingredient(&mut cmd, Id("nori".to_string()), &assets, &mut ingredients);
    let flour = add_ingredient(&mut cmd, Id("flour".to_string()), &assets, &mut ingredients);
    let cucumber = add_ingredient(&mut cmd, Id("cucumber".to_string()), &assets, &mut ingredients);
    let mayo = add_ingredient(&mut cmd, Id("mayo".to_string()), &assets, &mut ingredients);
    let cabbage = add_ingredient(&mut cmd, Id("cabbage".to_string()), &assets, &mut ingredients);
    let ketchup = add_ingredient(&mut cmd, Id("ketchup".to_string()), &assets, &mut ingredients);
    let broth = add_ingredient(&mut cmd, Id("broth".to_string()), &assets, &mut ingredients);
    let cheese = add_ingredient(&mut cmd, Id("cheese".to_string()), &assets, &mut ingredients);
    let popatoes = add_ingredient(&mut cmd, Id("potatoes".to_string()), &assets, &mut ingredients);
    let salami = add_ingredient(&mut cmd, Id("salami".to_string()), &assets, &mut ingredients);
    let meat = add_ingredient(&mut cmd, Id("meat".to_string()), &assets, &mut ingredients);
    let rice = add_ingredient(&mut cmd, Id("rice".to_string()), &assets, &mut ingredients);

    let (pane, pan) = add_processing(&mut cmd, Id("pan".to_string()), &assets, res);
    let (knifee, knife) = add_processing(&mut cmd, Id("knife".to_string()), &assets, res);
    let (pote, pot) = add_processing(&mut cmd, Id("pot".to_string()), &assets, res);
    //let (bowle, bowl) = add_processing(&mut cmd, Id("bowl".to_string()), &assets, res);
    let (toastere, toaster) = add_processing(&mut cmd, Id("toaster".to_string()), &assets, res);

    let mut keymap = KeyMapping(HashMap::new());
    keymap.0.insert(KeyCode::Key1, pane);
    keymap.0.insert(KeyCode::Key2, knifee);
    keymap.0.insert(KeyCode::Key3, pote);
    //keymap.0.insert(KeyCode::Key4, bowle);
    keymap.0.insert(KeyCode::Key4, toastere);

    cmd.insert_resource(keymap);

    add_food(&mut cmd,
        Id("fried_egg".to_string()),
        &assets,
        FoodIngredients(
            vec![IngredientProcessing { ingredient: egg.clone(), processing: pan.clone() }]
        ));

    add_food(&mut cmd,
        Id("soup".to_string()),
        &assets,
        FoodIngredients(
            vec![
                IngredientProcessing { ingredient: popatoes.clone(), processing: pot.clone() },
                IngredientProcessing { ingredient: broth.clone(), processing: pot.clone() },
                IngredientProcessing { ingredient: garlic.clone(), processing: knife.clone() },
            ]
        ));

    add_food(&mut cmd,
        Id("burger".to_string()),
        &assets,
        FoodIngredients(
            vec![
                IngredientProcessing { ingredient: bread.clone(), processing: toaster.clone() },
                IngredientProcessing { ingredient: cheese.clone(), processing: knife.clone() },
                IngredientProcessing { ingredient: cucumber.clone(), processing: knife.clone() },
                IngredientProcessing { ingredient: tomato.clone(), processing: knife.clone() },
                IngredientProcessing { ingredient: cabbage.clone(), processing: knife.clone() },
                IngredientProcessing { ingredient: meat.clone(), processing: pan.clone() },
                IngredientProcessing { ingredient: ketchup.clone(), processing: knife.clone() },
            ]
        ));

    add_food(&mut cmd,
        Id("sandwitch".to_string()),
        &assets,
        FoodIngredients(
            vec![
                IngredientProcessing { ingredient: bread.clone(), processing: toaster.clone() },
                IngredientProcessing { ingredient: cheese.clone(), processing: knife.clone() },
                IngredientProcessing { ingredient: salami.clone(), processing: knife.clone() },
                IngredientProcessing { ingredient: tomato.clone(), processing: knife.clone() },
                IngredientProcessing { ingredient: cabbage.clone(), processing: knife.clone() },
            ]
        ));

    add_food(&mut cmd,
        Id("sushi".to_string()),
        &assets,
        FoodIngredients(
            vec![
                IngredientProcessing { ingredient: nori.clone(), processing: knife.clone() },
                IngredientProcessing { ingredient: fish.clone(), processing: knife.clone() },
                IngredientProcessing { ingredient: rice.clone(), processing: pot.clone() },
            ]
        ));

    add_food(&mut cmd,
        Id("orange_cut".to_string()),
        &assets,
        FoodIngredients(
            vec![
                IngredientProcessing { ingredient: orange.clone(), processing: knife.clone() },
            ]
        ));

    cmd.insert_resource(ThrowConfig { time: 1., height: 100., drift: 100. });
}

fn loose(mut cmd: Commands, query: Query<Entity, (With<Food>, With<Active>)>, mut game_state: ResMut<NextState<GameState>>) {
    if query.iter().len() > 7 {
        for e in query.iter() { cmd.entity(e).despawn(); }
        game_state.set(GameState::MainMenu);
    }
}

fn spawn_foods(mut cmd: Commands, foods: Query<(Entity, &Tex, &FoodIngredients), With<Food>>, time: Res<Time>, mut timer: ResMut<FoodSpawnTimer>, score: Res<Score>) {
    if ! timer.0.tick(time.delta()).just_finished() { return; }

    timer.0.set_duration(Duration::from_secs_f32((FOOD_SPAWN - (score.0 as f32 * 0.05)).max(1.)));
    timer.0.reset();

    let (e, tex, ingredients) = foods.iter().choose(&mut rand::thread_rng()).unwrap();
    cmd.spawn((
        Active(e),
        Food{},
        ingredients.clone(),
        SpriteBundle {
            texture: tex.0.clone(),
            visibility: Visibility::Hidden,
            ..default()
        }
    ));
}

fn spawn_ingredients(mut cmd: Commands, time: Res<Time>, mut timer: ResMut<IngredientSpawnTimer>, cfg: Res<ThrowConfig>, foods: Query<&Active, With<Food>>, food_ingredients: Query<&FoodIngredients>, ingredients: Res<Ingredients>, tex: Query<&Tex, With<Ingredient>>, w: Query<&Window>, score: Res<Score>) {
    if ! timer.0.tick(time.delta()).just_finished() { return; }

    timer.0.set_duration(Duration::from_secs_f32((INGREDIENT_SPAWN - (score.0 as f32 * 0.3)).max(0.3)));
    timer.0.reset();

    let Some(food) = foods.iter().choose(&mut rand::thread_rng()) else { return };
    let ingredient_id = &food_ingredients.get(food.0).unwrap().0.choose(&mut rand::thread_rng()).unwrap().ingredient;
    let ingredient = ingredients.0.get(ingredient_id).unwrap();

    let w = w.single();
    let height = w.height() + w.height() / 2.;
    let drift: f32= rand::thread_rng().gen_range(-100. .. 100.);
    let time = (0.1 + (score.0 as f32 * 0.1)).min(5.);

    let g = height / 2.0 * time.powi(2);
    let v = f32::sqrt(2. * height * g);
    let drift = f32::sqrt(2. * drift.abs() * g) * drift.min(1.).max(-1.);

    let res = &w.resolution;
    let mut rng = rand::thread_rng();

    let spawn_x = rng.gen_range(- res.width() / 2. + 25. .. res.width() / 2. + 25.);
    let spawn_y = - res.height() / 2.;

    cmd.spawn((
        Active(*ingredient),
        Ingredient {},
        SpriteBundle {
            texture: tex.get(*ingredient).unwrap().0.clone(),
            transform: Transform::from_xyz(spawn_x, spawn_y, 10.),
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
