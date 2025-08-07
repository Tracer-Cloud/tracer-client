use rand::seq::IndexedRandom;
use rand::Rng;
use std::sync::LazyLock;

static ADJECTIVES: LazyLock<Vec<&str>> = LazyLock::new(|| {
    vec![
        "snowy", "silent", "desert", "mystic", "ancient", "gentle", "peaceful", "cheerful",
        "bright", "sunny", "cloudy", "misty", "crystal", "golden", "silver", "emerald",
        "sapphire", "ruby", "diamond", "pearl", "coral", "amber", "jade", "opal",
        "turquoise", "lavender", "violet", "azure", "crimson", "scarlet", "ivory",
        "marble", "velvet", "silk", "cotton", "linen", "wool", "satin", "fresh", "crisp",
        "pure", "clear", "transparent", "sparkling", "shimmering", "glowing",
        "radiant", "luminous", "brilliant", "dazzling", "gleaming", "polished", "refined",
        "elegant", "graceful", "delicate", "tender", "sweet", "mild", "calm", "serene",
        "tranquil", "quiet", "hushed", "whispering", "murmuring", "bubbling", "flowing",
        "dancing", "floating", "soaring", "gliding", "drifting", "wandering", "roaming",
        "exploring", "discovering", "curious", "playful", "joyful", "happy", "merry",
        "jolly", "festive", "celebratory", "triumphant", "victorious", "successful",
        "accomplished", "skilled", "talented", "gifted", "creative", "artistic", "musical",
        "poetic", "literary", "scholarly", "wise", "intelligent", "clever", "smart",
        "quick", "swift", "rapid", "speedy", "nimble", "agile", "flexible", "adaptable"
    ]
});
static ANIMALS: LazyLock<Vec<&str>> = LazyLock::new(|| {
    vec![
        "owl", "wolf", "lion", "tiger", "hawk", "eagle", "fox", "bear", "penguin", "dolphin",
        "elephant", "leopard", "giraffe", "rhino", "panther", "falcon", "lynx", "moose", "otter",
        "raccoon", "zebra", "cheetah", "jaguar", "puma", "cougar", "bobcat", "serval", "caracal",
        "ocelot", "kodiak", "grizzly", "polarbear", "panda", "koala", "kangaroo", "wallaby",
        "wombat", "platypus", "echidna", "sloth", "armadillo", "anteater", "tapir", "capybara",
        "chinchilla", "hamster", "gerbil", "ferret", "mink", "weasel", "stoat", "ermine",
        "badger", "wolverine", "skunk", "porcupine", "hedgehog", "shrew", "mole", "bat", "lemur",
         "mandrill", "macaque", "gibbon", "orangutan", "chimpanzee", "gorilla",
        "bonobo", "tarsier", "loris", "squirrel", "chipmunk", "marmot", "groundhog", "beaver",
        "muskrat", "vole",
        "lemming", "jerboa", "gopher", "mole", "rabbit", "hare", "pika", "deer", "elk", "caribou",
        "reindeer", "antelope", "gazelle", "impala", "springbok", "oryx", "kudu", "eland", "bison",
        "buffalo", "yak", "sheep", "goat", "ibex", "llama", "alpaca", "camel", "horse", "pony",
        "donkey", "mule", "okapi", "hippo", "pig", "boar", "warthog", "peccary",
        "aardvark", "pangolin", "manatee", "walrus", "seal", "whale", "narwhal",
        "beluga", "orca", "porpoise", "shark", "ray", "skate", "sturgeon", "salmon", "trout",
        "bass", "pike", "perch", "cod", "tuna", "marlin", "swordfish", "barracuda", "grouper",
        "snapper", "flounder", "sole", "halibut", "turbot", "plaice", "octopus", "squid",
        "cuttlefish", "nautilus", "jellyfish", "starfish", "urchin", "crab", "lobster", "shrimp",
        "turtle", "tortoise", "lizard", "gecko", "iguana", "chameleon", "komodo", "snake",
        "python", "boa", "anaconda", "viper", "cobra", "mamba", "adder", "rattlesnake",
        "frog", "toad", "salamander", "newt", "axolotl", "bullfrog", "treefrog", "crayfish"
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
