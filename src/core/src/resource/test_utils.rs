use image::{DynamicImage, GenericImageView};

/// Asserts that two images are equal
pub fn assert_images_are_equal(img_a: &DynamicImage, img_b: &DynamicImage) {
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
