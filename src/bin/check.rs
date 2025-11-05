use std::env;

use image_hasher::{HashAlg, HasherConfig};

fn main() {
    let hash_landscape_config = HasherConfig::new()
        .hash_size(30, 20)
        .hash_alg(HashAlg::Blockhash);
    let hasher_landscape = hash_landscape_config.to_hasher();

    let hash_portrait_config = HasherConfig::new()
        .hash_size(20, 30)
        .hash_alg(HashAlg::Blockhash);
    let hasher_portrait = hash_portrait_config.to_hasher();

    let hash_square_config = HasherConfig::new()
        .hash_size(30, 30)
        .hash_alg(HashAlg::Blockhash);
    let hasher_square = hash_square_config.to_hasher();

    let big_hasher_config = HasherConfig::new()
        .hash_size(100, 100)
        .hash_alg(HashAlg::Blockhash);
    let big_hasher = big_hasher_config.to_hasher();

    let hasher = hasher_square;

    let img1 = image::open(env::args().nth(1).expect("Failed to get 1st arg"))
        .expect("Failed to open 1st image");
    let img2 = image::open(env::args().nth(2).expect("Failed to get 2nd arg"))
        .expect("Failed to open 2nd image");
    let hash1 = hasher.hash_image(&img1);
    let hash2 = hasher.hash_image(&img2);

    println!(
        "Hash 1: {}, hash 2: {}",
        hash1.as_bytes().len(),
        hash2.as_bytes().len()
    );
    println!("Difference {}", hash1.dist(&hash2));
}
