//! Regenerates `src/builtin/builtin_data.rs` from Minecraft client textures.
//!
//! Usage:
//!   unzip client.jar 'assets/minecraft/textures/entity/*' -d ex
//!   cargo run --example gen_faces -- ex/assets/minecraft/textures/entity [--preview DIR]
//!
//! Each face is described as layers cut from texture files (the model's front
//! face UV) and composited onto a canvas, top layer last.

use mcface::image::Image;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

struct Layer {
    file: &'static str,
    /// Source rectangle (x, y, w, h) in the texture.
    src: (u32, u32, u32, u32),
    /// Destination offset on the canvas.
    dst: (i32, i32),
}

struct Spec {
    id: &'static str,
    name: &'static str,
    player: bool,
    size: (u32, u32),
    layers: Vec<Layer>,
    /// Make every non-transparent pixel fully opaque (for translucent mobs like slimes).
    solidify: bool,
    /// Box-filter shrink factor for high-resolution textures.
    downscale: u32,
}

fn layer(file: &'static str, src: (u32, u32, u32, u32), dst: (i32, i32)) -> Layer {
    Layer { file, src, dst }
}

/// Front of an 8x8x8 head at uv (0,0) plus its hat layer: the humanoid layout.
fn humanoid(file: &'static str) -> Vec<Layer> {
    vec![
        layer(file, (8, 8, 8, 8), (0, 0)),
        layer(file, (40, 8, 8, 8), (0, 0)),
    ]
}

fn mob(id: &'static str, name: &'static str, size: (u32, u32), layers: Vec<Layer>) -> Spec {
    Spec {
        id,
        name,
        player: false,
        size,
        layers,
        solidify: false,
        downscale: 1,
    }
}

/// Samples `rows` evenly spaced rows of `src` (nearest row), placing them one per
/// canvas row from `dst`. Used for faces seen at an angle, like a tilted snout.
fn squash(file: &'static str, src: (u32, u32, u32, u32), rows: u32, dst: (i32, i32)) -> Vec<Layer> {
    let (x, y, w, h) = src;
    (0..rows)
        .map(|i| {
            layer(
                file,
                (x, y + (2 * i + 1) * h / (2 * rows), w, 1),
                (dst.0, dst.1 + i as i32),
            )
        })
        .collect()
}

/// Villager-shaped head (8x10x8 at uv (0,0)) with its 2x4 nose hanging below the chin.
fn villager_head(file: &'static str) -> Vec<Layer> {
    vec![
        layer(file, (8, 8, 8, 10), (0, 0)),
        layer(file, (26, 2, 2, 4), (3, 7)),
    ]
}

/// Piglin-shaped head: 10x8x8 head, 4x4 snout, two 1x2 tusks.
fn piglin_head(file: &'static str) -> Vec<Layer> {
    vec![
        layer(file, (8, 8, 10, 8), (0, 0)),
        layer(file, (32, 2, 4, 4), (3, 4)),
        layer(file, (3, 5, 1, 2), (7, 6)),
        layer(file, (3, 1, 1, 2), (2, 6)),
    ]
}

/// Hoglin head pitched ~50° down: the top face (eyes) and snout front seen foreshortened,
/// with the two tusks rising from the snout sides.
fn hoglin_head(file: &'static str) -> Vec<Layer> {
    let mut l = squash(file, (80, 1, 14, 19), 15, (1, 0));
    l.extend(squash(file, (80, 20, 14, 6), 4, (1, 15)));
    l.extend(squash(file, (12, 15, 2, 11), 7, (0, 5)));
    l.extend(squash(file, (3, 15, 2, 11), 7, (14, 5)));
    l
}

fn specs() -> Vec<Spec> {
    let mut v = Vec::new();
    // Default player skins. The face is identical for wide and slim models.
    for (id, name) in [
        ("steve", "Steve"),
        ("alex", "Alex"),
        ("ari", "Ari"),
        ("efe", "Efe"),
        ("kai", "Kai"),
        ("makena", "Makena"),
        ("noor", "Noor"),
        ("sunny", "Sunny"),
        ("zuri", "Zuri"),
    ] {
        let file: &'static str = Box::leak(format!("player/wide/{id}.png").into_boxed_str());
        v.push(Spec {
            id,
            name,
            player: true,
            size: (8, 8),
            layers: humanoid(file),
            solidify: false,
            downscale: 1,
        });
    }

    v.push(mob(
        "creeper",
        "Creeper",
        (8, 8),
        vec![layer("creeper/creeper.png", (8, 8, 8, 8), (0, 0))],
    ));
    v.push(mob(
        "zombie",
        "Zombie",
        (8, 8),
        humanoid("zombie/zombie.png"),
    ));
    v.push(mob("husk", "Husk", (8, 8), humanoid("zombie/husk.png")));
    v.push(mob("drowned", "Drowned", (8, 8), {
        let mut l = humanoid("zombie/drowned.png");
        l.extend(humanoid("zombie/drowned_outer_layer.png"));
        l
    }));
    v.push(mob(
        "skeleton",
        "Skeleton",
        (8, 8),
        humanoid("skeleton/skeleton.png"),
    ));
    v.push(mob("stray", "Stray", (8, 8), {
        let mut l = humanoid("skeleton/stray.png");
        l.extend(humanoid("skeleton/stray_overlay.png"));
        l
    }));
    v.push(mob("bogged", "Bogged", (8, 8), {
        let mut l = humanoid("skeleton/bogged.png");
        l.extend(humanoid("skeleton/bogged_overlay.png"));
        l
    }));
    v.push(mob(
        "wither_skeleton",
        "Wither Skeleton",
        (8, 8),
        humanoid("skeleton/wither_skeleton.png"),
    ));
    // Enderman: the head texture leaves its bottom rows empty; the jaw box at uv (0,16)
    // fills them in behind it.
    v.push(mob(
        "enderman",
        "Enderman",
        (8, 8),
        vec![
            layer("enderman/enderman.png", (8, 24, 8, 8), (0, 0)),
            layer("enderman/enderman.png", (8, 8, 8, 8), (0, 0)),
            layer("enderman/enderman_eyes.png", (8, 8, 8, 8), (0, 0)),
        ],
    ));
    // Spider head: 8x8x8 box at uv (32,4).
    for (id, name, file) in [
        ("spider", "Spider", "spider/spider.png"),
        ("cave_spider", "Cave Spider", "spider/cave_spider.png"),
    ] {
        v.push(mob(
            id,
            name,
            (8, 8),
            vec![
                layer(file, (40, 12, 8, 8), (0, 0)),
                layer("spider/spider_eyes.png", (40, 12, 8, 8), (0, 0)),
            ],
        ));
    }
    // Pig: head at uv (0,0), 4x3x1 snout at uv (16,16) sitting at the lower middle.
    v.push(mob(
        "pig",
        "Pig",
        (8, 8),
        vec![
            layer("pig/pig_temperate.png", (8, 8, 8, 8), (0, 0)),
            layer("pig/pig_temperate.png", (17, 17, 4, 3), (2, 4)),
        ],
    ));
    // Cow: 8x8x6 head at uv (0,0), 6x3 muzzle front at (2,34) across the bottom.
    v.push(mob(
        "cow",
        "Cow",
        (8, 8),
        vec![
            layer("cow/cow_temperate.png", (6, 6, 8, 8), (0, 0)),
            layer("cow/cow_temperate.png", (2, 34, 6, 3), (1, 5)),
        ],
    ));
    // Villager: 8x10x8 head at uv (0,0), 2x4x2 nose at uv (24,0) hanging below the chin.
    v.push(mob(
        "villager",
        "Villager",
        (8, 11),
        villager_head("villager/villager.png"),
    ));
    // Iron golem: same head/nose shape as the villager.
    v.push(mob(
        "iron_golem",
        "Iron Golem",
        (8, 11),
        vec![
            layer("iron_golem/iron_golem.png", (8, 8, 8, 10), (0, 0)),
            layer("iron_golem/iron_golem.png", (26, 2, 2, 4), (3, 7)),
        ],
    ));
    // Piglin: 10x8x8 head, 4x4 snout, two 1x2 tusks.
    v.push(mob(
        "piglin",
        "Piglin",
        (10, 8),
        piglin_head("piglin/piglin.png"),
    ));
    v.push(mob(
        "blaze",
        "Blaze",
        (8, 8),
        vec![layer("blaze/blaze.png", (8, 8, 8, 8), (0, 0))],
    ));
    // Ghast: the whole body is the face. The texture is double resolution, so halve it.
    v.push(Spec {
        downscale: 2,
        ..mob(
            "ghast",
            "Ghast",
            (32, 32),
            vec![layer("ghast/ghast.png", (32, 32, 32, 32), (0, 0))],
        )
    });
    // Slime: the translucent outer cube, then the inner cube, eyes and mouth drawn on top
    // so they stay readable instead of being washed out by the outer layer.
    v.push(Spec {
        solidify: true,
        ..mob(
            "slime",
            "Slime",
            (8, 8),
            vec![
                layer("slime/slime.png", (8, 8, 8, 8), (0, 0)),
                layer("slime/slime.png", (6, 22, 6, 6), (1, 1)),
                layer("slime/slime.png", (34, 2, 2, 2), (1, 2)),
                layer("slime/slime.png", (34, 6, 2, 2), (5, 2)),
                layer("slime/slime.png", (33, 9, 1, 1), (4, 5)),
            ],
        )
    });

    // Friendly and neutral mobs.
    // Sheep: 6x6 face at (8,8); the inflated wool head box shows as a 1px frame, approximated
    // by tiling the wool front (6,6) behind the face.
    v.push(mob(
        "sheep",
        "Sheep",
        (8, 8),
        vec![
            layer("sheep/sheep_wool.png", (6, 6, 6, 6), (0, 0)),
            layer("sheep/sheep_wool.png", (6, 6, 6, 6), (2, 0)),
            layer("sheep/sheep_wool.png", (6, 6, 6, 6), (0, 2)),
            layer("sheep/sheep_wool.png", (6, 6, 6, 6), (2, 2)),
            layer("sheep/sheep.png", (8, 8, 6, 6), (1, 1)),
        ],
    ));
    // Chicken: 4x6x3 head at uv (0,0), 4x2 beak front at (16,2), 2x2 wattle front at (16,6).
    v.push(mob(
        "chicken",
        "Chicken",
        (4, 6),
        vec![
            layer("chicken/chicken_temperate.png", (3, 3, 4, 6), (0, 0)),
            layer("chicken/chicken_temperate.png", (16, 2, 4, 2), (0, 2)),
            layer("chicken/chicken_temperate.png", (16, 6, 2, 2), (1, 4)),
        ],
    ));
    // Wolf: 6x6x4 head at uv (0,0), 2x2 ears (front at (17,15)) above, 3x3 snout front at (4,14).
    v.push(mob(
        "wolf",
        "Wolf",
        (6, 8),
        vec![
            layer("wolf/wolf.png", (17, 15, 2, 2), (0, 0)),
            layer("wolf/wolf.png", (17, 15, 2, 2), (4, 0)),
            layer("wolf/wolf.png", (4, 4, 6, 6), (0, 2)),
            layer("wolf/wolf.png", (4, 14, 3, 3), (1, 5)),
        ],
    ));
    // Cat / ocelot: 5x4x5 head at uv (0,0), 1x1 ears (fronts at (2,12), (8,12)) above, 3x2 nose front at (2,26).
    for (id, name, file) in [
        ("cat", "Cat", "cat/cat_tabby.png"),
        ("ocelot", "Ocelot", "cat/ocelot.png"),
    ] {
        v.push(mob(
            id,
            name,
            (5, 5),
            vec![
                layer(file, (2, 12, 1, 1), (1, 0)),
                layer(file, (8, 12, 1, 1), (3, 0)),
                layer(file, (5, 5, 5, 4), (0, 1)),
                layer(file, (2, 26, 3, 2), (1, 3)),
            ],
        ));
    }
    // Fox: 8x6x6 head at uv (1,5) (eyes wrap the corners), 2x2 ears at (9,2)/(16,2), 4x2 nose front at (9,21).
    v.push(mob(
        "fox",
        "Fox",
        (8, 8),
        vec![
            layer("fox/fox.png", (9, 2, 2, 2), (0, 0)),
            layer("fox/fox.png", (16, 2, 2, 2), (6, 0)),
            layer("fox/fox.png", (7, 11, 8, 6), (0, 2)),
            layer("fox/fox.png", (9, 21, 4, 2), (2, 6)),
        ],
    ));
    // Rabbit (26.x 64x64 layout): 5x5x5 head at uv (0,16), 2x5x1 ears at uv (26,0)/(32,0) standing on top.
    v.push(mob(
        "rabbit",
        "Rabbit",
        (5, 10),
        vec![
            layer("rabbit/rabbit_brown.png", (27, 1, 2, 5), (0, 0)),
            layer("rabbit/rabbit_brown.png", (33, 1, 2, 5), (3, 0)),
            layer("rabbit/rabbit_brown.png", (5, 21, 5, 5), (0, 5)),
        ],
    ));
    // Panda: 13x10x9 head at uv (0,6), 7x5 muzzle front at (47,18), 5x4 ears (front (53,26)) sticking out 2px each side.
    v.push(mob(
        "panda",
        "Panda",
        (17, 13),
        vec![
            layer("panda/panda.png", (53, 26, 5, 4), (0, 0)),
            layer("panda/panda.png", (53, 26, 5, 4), (12, 0)),
            layer("panda/panda.png", (9, 15, 13, 10), (2, 3)),
            layer("panda/panda.png", (47, 18, 7, 5), (5, 8)),
        ],
    ));
    // Polar bear: 7x7x7 head at uv (0,0), 2x2 ears (front (27,1)) at the top corners, 5x3 muzzle front at (3,47).
    v.push(mob(
        "polar_bear",
        "Polar Bear",
        (9, 8),
        vec![
            layer("bear/polarbear.png", (27, 1, 2, 2), (0, 0)),
            layer("bear/polarbear.png", (27, 1, 2, 2), (7, 0)),
            layer("bear/polarbear.png", (7, 7, 7, 7), (1, 1)),
            layer("bear/polarbear.png", (3, 47, 5, 3), (2, 5)),
        ],
    ));
    // Bee: front of the 7x7x10 body at uv (0,0); eyes wrap the corners.
    v.push(mob(
        "bee",
        "Bee",
        (7, 7),
        vec![layer("bee/bee.png", (10, 10, 7, 7), (0, 0))],
    ));
    // Axolotl: 8x5x5 head at uv (0,1), flat gill planes: top 8x3 at (3,37), sides 3x7 at (0,40)/(11,40).
    v.push(mob(
        "axolotl",
        "Axolotl",
        (14, 8),
        vec![
            layer("axolotl/axolotl_lucy.png", (3, 37, 8, 3), (3, 0)),
            layer("axolotl/axolotl_lucy.png", (0, 40, 3, 7), (0, 1)),
            layer("axolotl/axolotl_lucy.png", (11, 40, 3, 7), (11, 1)),
            layer("axolotl/axolotl_lucy.png", (5, 6, 8, 5), (3, 3)),
        ],
    ));
    // Frog: 3x2 eye fronts at (3,3)/(3,8) on top, 7x3 upper head front at (9,22) over the 7x3 body front at (12,10).
    v.push(mob(
        "frog",
        "Frog",
        (7, 7),
        vec![
            layer("frog/frog_temperate.png", (3, 3, 3, 2), (0, 0)),
            layer("frog/frog_temperate.png", (3, 8, 3, 2), (4, 0)),
            layer("frog/frog_temperate.png", (12, 10, 7, 3), (0, 4)),
            layer("frog/frog_temperate.png", (9, 22, 7, 3), (0, 2)),
        ],
    ));
    // Turtle: front of the 6x5x6 head at uv (3,0); its eyes are on the sides, so none show from the front.
    v.push(mob(
        "turtle",
        "Turtle",
        (6, 5),
        vec![layer("turtle/turtle.png", (9, 6, 6, 5), (0, 0))],
    ));
    // Mooshroom: same layout as the cow (8x8x6 head, 6x3 muzzle front at (2,34)).
    v.push(mob(
        "mooshroom",
        "Mooshroom",
        (8, 8),
        vec![
            layer("cow/mooshroom_red.png", (6, 6, 8, 8), (0, 0)),
            layer("cow/mooshroom_red.png", (2, 34, 6, 3), (1, 5)),
        ],
    ));
    // Goat: 2x7 horns (front (14,57)) and 3x2 ears (front (3,62)); the 5x7x10 head at uv (34,46) is tilted
    // ~55° down, so from the front its top face (44,46) shows, trimmed to the front 8 rows to mimic foreshortening.
    v.push(mob(
        "goat",
        "Goat",
        (11, 15),
        vec![
            layer("goat/goat.png", (14, 57, 2, 7), (3, 0)),
            layer("goat/goat.png", (14, 57, 2, 7), (6, 0)),
            layer("goat/goat.png", (3, 62, 3, 2), (0, 5)),
            layer("goat/goat.png", (3, 62, 3, 2), (8, 5)),
            layer("goat/goat.png", (44, 48, 5, 8), (3, 7)),
        ],
    ));
    // Llama: 8-wide head/neck front at (6,20) (top 10 rows), 3x3 ears above, 4x4 snout over rows 2-5.
    v.push(mob(
        "llama",
        "Llama",
        (8, 13),
        vec![
            layer("llama/llama_creamy.png", (19, 2, 3, 3), (0, 0)),
            layer("llama/llama_creamy.png", (19, 2, 3, 3), (5, 0)),
            layer("llama/llama_creamy.png", (6, 20, 8, 10), (0, 3)),
            layer("llama/llama_creamy.png", (9, 9, 4, 4), (2, 5)),
        ],
    ));
    // Horse: 6x5 head front with the 4x5 muzzle in front of it, small 2x3 ears behind.
    v.push(mob(
        "horse",
        "Horse",
        (6, 7),
        vec![
            layer("horse/horse_creamy.png", (20, 17, 2, 3), (0, 0)),
            layer("horse/horse_creamy.png", (20, 17, 2, 3), (4, 0)),
            layer("horse/horse_creamy.png", (7, 20, 6, 5), (0, 2)),
            layer("horse/horse_creamy.png", (5, 30, 4, 5), (1, 2)),
        ],
    ));
    // Donkey: horse head layout with long 2x7 ears at uv (0,12).
    v.push(mob(
        "donkey",
        "Donkey",
        (6, 10),
        vec![
            layer("horse/donkey.png", (1, 13, 2, 7), (0, 0)),
            layer("horse/donkey.png", (1, 13, 2, 7), (4, 0)),
            layer("horse/donkey.png", (7, 20, 6, 5), (0, 5)),
            layer("horse/donkey.png", (5, 30, 4, 5), (1, 5)),
        ],
    ));
    // Parrot: 2x3 head front plus the side columns that hold the eyes, crest front above,
    // beak widened to 2px so it sits centred (the real 1px beak straddles the centre).
    v.push(mob(
        "parrot",
        "Parrot",
        (4, 5),
        vec![
            layer("parrot/parrot_red_blue.png", (14, 4, 2, 1), (1, 0)),
            layer("parrot/parrot_red_blue.png", (3, 4, 4, 3), (0, 1)),
            layer("parrot/parrot_red_blue.png", (12, 8, 1, 2), (1, 2)),
            layer("parrot/parrot_red_blue.png", (12, 8, 1, 2), (2, 2)),
            layer("parrot/parrot_red_blue.png", (17, 8, 1, 1), (1, 4)),
            layer("parrot/parrot_red_blue.png", (17, 8, 1, 1), (2, 4)),
        ],
    ));
    // Squid: 12x16 body front (eyes on it), five front tentacle stubs (2x4 at uv front (50,2)) below.
    for (id, name, file) in [
        ("squid", "Squid", "squid/squid.png"),
        ("glow_squid", "Glow Squid", "squid/glow_squid.png"),
    ] {
        v.push(mob(
            id,
            name,
            (12, 20),
            vec![
                layer(file, (50, 2, 2, 4), (0, 16)),
                layer(file, (50, 2, 2, 4), (10, 16)),
                layer(file, (50, 2, 2, 4), (2, 16)),
                layer(file, (50, 2, 2, 4), (8, 16)),
                layer(file, (50, 2, 2, 4), (5, 16)),
                layer(file, (12, 12, 12, 16), (0, 0)),
            ],
        ));
    }
    // Dolphin: 8x7 head front, 2x2 nose (uv (0,13), 2x2x4) at the lower middle.
    v.push(mob(
        "dolphin",
        "Dolphin",
        (8, 7),
        vec![
            layer("dolphin/dolphin.png", (6, 6, 8, 7), (0, 0)),
            layer("dolphin/dolphin.png", (4, 17, 2, 2), (3, 5)),
        ],
    ));
    // Bat: 4x3 head (uv (0,7)) plus the side columns with the white eyes, 2x4 ears above.
    v.push(mob(
        "bat",
        "Bat",
        (6, 6),
        vec![
            layer("bat/bat.png", (2, 16, 2, 4), (1, 0)),
            layer("bat/bat.png", (10, 16, 2, 4), (3, 0)),
            layer("bat/bat.png", (1, 9, 6, 3), (0, 3)),
        ],
    ));
    // Snow golem: 8x8 snow head with coal face (sheared; the pumpkin is a block texture).
    v.push(mob(
        "snow_golem",
        "Snow Golem",
        (8, 8),
        vec![layer("snow_golem/snow_golem.png", (8, 8, 8, 8), (0, 0))],
    ));
    // Allay: 5x5x5 head at uv (0,0).
    v.push(mob(
        "allay",
        "Allay",
        (5, 5),
        vec![layer("allay/allay.png", (5, 5, 5, 5), (0, 0))],
    ));
    // Strider: the 16x14x16 body is the face.
    v.push(mob(
        "strider",
        "Strider",
        (16, 14),
        vec![layer("strider/strider.png", (16, 16, 16, 14), (0, 0))],
    ));
    // Sniffer: 13-wide head front (only its top rows are opaque), 13x12 beak and 13x2 nose
    // seen through it, 1px-wide hanging ears at both sides.
    v.push(mob(
        "sniffer",
        "Sniffer",
        (15, 16),
        vec![
            layer("sniffer/sniffer.png", (9, 7, 1, 16), (0, 0)),
            layer("sniffer/sniffer.png", (55, 7, 1, 16), (14, 0)),
            layer("sniffer/sniffer.png", (19, 26, 13, 16), (1, 0)),
            layer("sniffer/sniffer.png", (19, 66, 13, 12), (1, 4)),
            layer("sniffer/sniffer.png", (19, 54, 13, 2), (1, 2)),
        ],
    ));
    // Armadillo: 3x5 snout-head front plus eye-bearing side columns, 2x4 ears above.
    v.push(mob(
        "armadillo",
        "Armadillo",
        (5, 8),
        vec![
            layer("armadillo/armadillo.png", (43, 10, 2, 4), (0, 0)),
            layer("armadillo/armadillo.png", (47, 10, 2, 4), (3, 0)),
            layer("armadillo/armadillo.png", (44, 17, 5, 5), (0, 3)),
        ],
    ));
    // Camel: top of the 7-wide upper neck front, 5x5 snout over it, 3x1 ears sticking out sideways.
    v.push(mob(
        "camel",
        "Camel",
        (13, 8),
        vec![
            layer("camel/camel.png", (47, 2, 3, 1), (0, 1)),
            layer("camel/camel.png", (47, 2, 3, 1), (10, 1)),
            layer("camel/camel.png", (28, 7, 7, 8), (3, 0)),
            layer("camel/camel.png", (56, 6, 5, 5), (4, 0)),
        ],
    ));
    // Copper golem: 4x4 rod knob on a 2x3 rod, 8x5 head with emissive eyes, 2x4 nose hanging below.
    v.push(mob(
        "copper_golem",
        "Copper Golem",
        (8, 13),
        vec![
            layer("copper_golem/copper_golem.png", (41, 4, 4, 4), (2, 0)),
            layer("copper_golem/copper_golem.png", (59, 2, 2, 3), (3, 4)),
            layer("copper_golem/copper_golem.png", (10, 10, 8, 5), (0, 7)),
            layer("copper_golem/copper_golem_eyes.png", (10, 10, 8, 5), (0, 7)),
            layer("copper_golem/copper_golem.png", (39, 10, 2, 4), (3, 9)),
        ],
    ));
    // Wandering trader: villager head/nose layout plus the hood front (open in the middle).
    v.push(mob(
        "wandering_trader",
        "Wandering Trader",
        (8, 11),
        vec![
            layer(
                "wandering_trader/wandering_trader.png",
                (8, 8, 8, 10),
                (0, 0),
            ),
            layer(
                "wandering_trader/wandering_trader.png",
                (40, 8, 8, 10),
                (0, 0),
            ),
            layer(
                "wandering_trader/wandering_trader.png",
                (26, 2, 2, 4),
                (3, 7),
            ),
        ],
    ));

    // More hostile mobs.
    // Witch: villager head and nose (with its mole) under the layered hat: 10x2 brim,
    // 7x4 crown, 4x4 upper crown and 1x2 tip.
    v.push(mob("witch", "Witch", (10, 21), {
        let mut l: Vec<Layer> = villager_head("witch/witch.png")
            .into_iter()
            .map(|l| Layer {
                dst: (l.dst.0 + 1, l.dst.1 + 10),
                ..l
            })
            .collect();
        l.push(layer("witch/witch.png", (1, 1, 1, 1), (5, 19)));
        l.push(layer("witch/witch.png", (10, 74, 10, 2), (0, 10)));
        l.push(layer("witch/witch.png", (7, 83, 7, 4), (2, 6)));
        l.push(layer("witch/witch.png", (4, 91, 4, 4), (4, 2)));
        l.push(layer("witch/witch.png", (1, 96, 1, 2), (5, 0)));
        l
    }));
    // Illagers and the zombie villager share the villager head shape.
    for (id, name, file) in [
        ("pillager", "Pillager", "illager/pillager.png"),
        ("vindicator", "Vindicator", "illager/vindicator.png"),
        ("evoker", "Evoker", "illager/evoker.png"),
        (
            "zombie_villager",
            "Zombie Villager",
            "zombie_villager/zombie_villager.png",
        ),
    ] {
        v.push(mob(id, name, (8, 11), villager_head(file)));
    }
    // Zombified piglin and piglin brute use the piglin head layout.
    v.push(mob(
        "zombified_piglin",
        "Zombified Piglin",
        (10, 8),
        piglin_head("piglin/zombified_piglin.png"),
    ));
    v.push(mob(
        "piglin_brute",
        "Piglin Brute",
        (10, 8),
        piglin_head("piglin/piglin_brute.png"),
    ));
    // Hoglin/zoglin: 14x6x19 head at uv (61,1), pitched down, tusks at the sides.
    v.push(mob(
        "hoglin",
        "Hoglin",
        (16, 19),
        hoglin_head("hoglin/hoglin.png"),
    ));
    v.push(mob(
        "zoglin",
        "Zoglin",
        (16, 19),
        hoglin_head("hoglin/zoglin.png"),
    ));
    // Magma cube: eight stacked 8x1x8 slices; slice i's front row comes from uv
    // (0, 9i) for the top four and (32, 9(i-4)) for the bottom four.
    v.push(mob("magma_cube", "Magma Cube", (8, 8), {
        (0..8u32)
            .map(|i| {
                let (u, v) = if i < 4 { (0, 9 * i) } else { (32, 9 * (i - 4)) };
                layer("slime/magmacube.png", (u + 8, v + 8, 8, 1), (0, i as i32))
            })
            .collect()
    }));
    // Guardian: 12x12x16 body with 2x12 side plates, 12x2 top/bottom plates and a 2x2 eye.
    for (id, name, file) in [
        ("guardian", "Guardian", "guardian/guardian.png"),
        (
            "elder_guardian",
            "Elder Guardian",
            "guardian/guardian_elder.png",
        ),
    ] {
        v.push(mob(
            id,
            name,
            (16, 16),
            vec![
                layer(file, (16, 16, 12, 12), (2, 2)),
                layer(file, (12, 40, 2, 12), (0, 2)),
                layer(file, (12, 40, 2, 12), (14, 2)),
                layer(file, (28, 52, 12, 2), (2, 0)),
                layer(file, (28, 52, 12, 2), (2, 14)),
                layer(file, (9, 1, 2, 2), (7, 7)),
            ],
        ));
    }
    // Warden: 16x16x10 head at uv (0,32) with the glowing layer, and the two
    // 16x16 tendril planes sticking out above the sides.
    v.push(mob("warden", "Warden", (36, 19), {
        let mut l = Vec::new();
        for file in [
            "warden/warden.png",
            "warden/warden_bioluminescent_layer.png",
        ] {
            l.push(layer(file, (52, 32, 16, 16), (-6, -6)));
            l.push(layer(file, (58, 0, 16, 16), (26, -6)));
            l.push(layer(file, (10, 42, 16, 16), (10, 3)));
        }
        l
    }));
    // Breeze: 8x8x8 head; the eyes wrap around the front corners.
    v.push(mob(
        "breeze",
        "Breeze",
        (8, 8),
        vec![
            layer("breeze/breeze.png", (8, 8, 8, 8), (0, 0)),
            layer("breeze/breeze_eyes.png", (8, 8, 8, 8), (0, 0)),
        ],
    ));
    // Shulker (undyed): 16x12 lid lifted 4px so the 6x6 head peeks through the notch
    // in its rim, above the 16x8 base whose tab covers the chin.
    v.push(mob(
        "shulker",
        "Shulker",
        (16, 20),
        vec![
            layer("shulker/shulker.png", (6, 58, 6, 6), (5, 8)),
            layer("shulker/shulker.png", (16, 16, 16, 12), (0, 0)),
            layer("shulker/shulker.png", (16, 44, 16, 8), (0, 12)),
        ],
    ));
    // Phantom: 7x3 head with glowing eyes in front of the body and 6x2 wing roots.
    v.push(mob(
        "phantom",
        "Phantom",
        (17, 4),
        vec![
            layer("phantom/phantom.png", (32, 21, 6, 2), (0, 0)),
            layer("phantom/phantom.png", (32, 21, 6, 2), (11, 0)),
            layer("phantom/phantom.png", (9, 17, 5, 3), (6, 0)),
            layer("phantom/phantom.png", (5, 5, 7, 3), (5, 1)),
            layer("phantom/phantom_eyes.png", (5, 5, 7, 3), (5, 1)),
        ],
    ));
    // Vex: 5x5x5 head at uv (0,0).
    v.push(mob(
        "vex",
        "Vex",
        (5, 5),
        vec![layer("illager/vex.png", (5, 5, 5, 5), (0, 0))],
    ));
    // Ravager: 16x20x16 head, 4x8 nose sticking out below it, 16x3 lower jaw and
    // two 2x14 horns tilted back beside the head.
    v.push(mob("ravager", "Ravager", (20, 25), {
        let file = "illager/ravager.png";
        let mut l = vec![
            layer(file, (16, 16, 16, 20), (2, 0)),
            layer(file, (16, 52, 16, 3), (2, 22)),
            layer(file, (4, 4, 4, 8), (8, 14)),
        ];
        l.extend(squash(file, (78, 59, 2, 14), 7, (0, 0)));
        l.extend(squash(file, (78, 59, 2, 14), 7, (18, 0)));
        l
    }));
    v
}

fn load(root: &Path, file: &str) -> Image {
    let bytes = std::fs::read(root.join(file)).unwrap_or_else(|e| panic!("{file}: {e}"));
    Image::decode_png(&bytes).unwrap_or_else(|e| panic!("{file}: {e}"))
}

fn build(root: &Path, spec: &Spec) -> Image {
    let mut canvas = Image::new(spec.size.0, spec.size.1);
    for (i, l) in spec.layers.iter().enumerate() {
        let tex = load(root, l.file);
        let (x, y, w, h) = l.src;
        let mut part = tex
            .crop(x, y, w, h)
            .unwrap_or_else(|e| panic!("{}: {}: {e}", spec.id, l.file));
        // Minecraft renders the base head layer of player skins fully opaque.
        if spec.player && i == 0 {
            part.px.iter_mut().for_each(|p| p[3] = 255);
        }
        canvas.over(&part, l.dst.0, l.dst.1);
    }
    if spec.solidify {
        canvas
            .px
            .iter_mut()
            .filter(|p| p[3] > 0)
            .for_each(|p| p[3] = 255);
    }
    shrink(&canvas, spec.downscale)
}

fn shrink(img: &Image, k: u32) -> Image {
    let mut out = Image::new(img.w / k, img.h / k);
    for y in 0..out.h {
        for x in 0..out.w {
            let mut sum = [0u32; 4];
            for dy in 0..k {
                for dx in 0..k {
                    let p = img.get(x * k + dx, y * k + dy);
                    for i in 0..4 {
                        sum[i] += p[i] as u32;
                    }
                }
            }
            let n = k * k;
            out.set(x, y, sum.map(|s| (s / n) as u8));
        }
    }
    out
}

fn main() {
    let mut args = std::env::args().skip(1);
    let root = PathBuf::from(
        args.next()
            .expect("usage: gen_faces <textures/entity dir> [--preview DIR]"),
    );
    let preview = match args.next().as_deref() {
        Some("--preview") => Some(PathBuf::from(
            args.next().expect("--preview needs a directory"),
        )),
        _ => None,
    };

    let mut out = String::from(
        "// @generated by `cargo run --example gen_faces` from Minecraft client textures. Do not edit.\n\n\
         use super::{Builtin, Kind};\n\n\
         pub static BUILTINS: &[Builtin] = &[\n",
    );
    for spec in specs() {
        let face = build(&root, &spec);
        if let Some(dir) = &preview {
            std::fs::create_dir_all(dir).unwrap();
            std::fs::write(
                dir.join(format!("{}.png", spec.id)),
                face.scale(16).encode_png(),
            )
            .unwrap();
        }
        let kind = if spec.player { "Player" } else { "Mob" };
        writeln!(
            out,
            "    Builtin {{ id: {:?}, name: {:?}, kind: Kind::{kind}, w: {}, h: {}, rgba: &[",
            spec.id, spec.name, face.w, face.h
        )
        .unwrap();
        for row in face.px.chunks(face.w as usize) {
            let bytes: Vec<String> = row.iter().flatten().map(|b| format!("0x{b:02x}")).collect();
            writeln!(out, "        {},", bytes.join(", ")).unwrap();
        }
        out.push_str("    ] },\n");
    }
    out.push_str("];\n");

    let dest = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/builtin/builtin_data.rs");
    std::fs::write(&dest, out).unwrap();
    eprintln!("wrote {}", dest.display());
}
