use std::path::Path;

use image::GenericImageView;


    /// Asserts that two png images are equal
    pub fn assert_png_images_are_equal<A: AsRef<Path>, B: AsRef<Path>>(path_a: A, path_b: B) {
        let img_a = image::open(path_a).unwrap();
        let img_b = image::open(path_b).unwrap();

        if img_a.dimensions() != img_b.dimensions() {
            panic!("Images dimensions are different");
        }

        if img_a.color() != img_b.color() {
            panic!("Images colors are different");
        }

        if img_a.to_rgba8() != img_b.to_rgba8() {
            panic!("Images bytes are different");
        }
    }