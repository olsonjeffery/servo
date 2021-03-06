export Image;

export load;
export load_from_memory;
export test_image_bin;

use stb_image = stb_image::image;

// FIXME: Images must not be copied every frame. Instead we should atomically
// reference count them.

type Image = stb_image::Image;

fn Image(width: uint, height: uint, depth: uint, +data: ~[u8]) -> Image {
    stb_image::new_image(width, height, depth, data)
}

const TEST_IMAGE: [u8 * 4962] = #include_bin("test.jpeg");

fn test_image_bin() -> ~[u8] {
    return vec::from_fn(4962, |i| TEST_IMAGE[i]);
}

fn load_from_memory(buffer: &[u8]) -> Option<Image> {
    do stb_image::load_from_memory(buffer).map |image| {

        assert image.depth == 4;
        // Do color space conversion :(
        let data = do vec::from_fn(image.width * image.height * 4) |i| {
            let color = i % 4;
            let pixel = i / 4;
            match color {
              0 => image.data[pixel * 4 + 2],
              1 => image.data[pixel * 4 + 1],
              2 => image.data[pixel * 4 + 0],
              3 => 0xffu8,
              _ => fail
            }
        };

        assert image.data.len() == data.len();

       Image(image.width, image.height, image.depth, data)
    }
}
