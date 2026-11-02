use pixel_engine::raster::arc::{Box2, arc_mask};

fn runs(row: &[u8]) -> String {
    let mut out = String::new();
    let mut i = 0;
    while i < row.len() {
        let v = row[i];
        let start = i;
        while i < row.len() && row[i] == v {
            i += 1;
        }
        out.push_str(&format!(
            "{}{}..{}, ",
            if v == 1 { "#" } else { "." },
            start,
            i - 1
        ));
    }
    out
}

fn main() {
    let b = Box2 {
        cx: 32.0,
        cy: 32.0,
        rx: 30.0,
        ry: 15.0,
    };
    let m = arc_mask(b, 0.0, 180.0, 12, 66, 60).unwrap();
    for y in 28..56 {
        println!("y={y:3}: {}", runs(&m.data[y * m.w..(y + 1) * m.w]));
    }
    println!("\ncolumn runs for x=20: {}", {
        let col: Vec<u8> = (0..m.h).map(|y| m.data[y * m.w + 20]).collect();
        runs(&col)
    });
}
