use rand::seq::IndexedRandom;
use rand::Rng;
use std::sync::LazyLock;

static ADJECTIVES: LazyLock<Vec<&str>> =
    LazyLock::new(|| vec!["snowy", "silent", "desert", "mystic", "ancient"]);
static ANIMALS: LazyLock<Vec<&str>> = LazyLock::new(|| {
    vec![
        "owl", "wolf", "lion", "tiger", "hawk", "eagle", "fox", "bear", "penguin", "dolphin",
        "elephant", "leopard", "giraffe", "rhino", "panther", "falcon", "lynx", "moose", "otter",
        "raccoon",
    ]
});

pub(super) fn generate_run_name() -> String {
    let mut rng = rand::rng();
    let adjective = ADJECTIVES.choose(&mut rng).unwrap();
    let animal = ANIMALS.choose(&mut rng).unwrap();
    let random_number = rng.random_range(0..1000);

    format!("{}-{}-{:03}", adjective, animal, random_number)
}

pub(super) fn generate_run_id() -> String {
    uuid::Uuid::new_v4().to_string()
}
