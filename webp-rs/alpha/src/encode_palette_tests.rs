use super::*;

#[test]
fn packed_indices_reconstruct_each_source_row() {
    let samples = (0..5)
        .flat_map(|row| (0..13).map(move |column| ((row + column) % 4) as u8 * 17))
        .collect::<Vec<_>>();
    let plan = make_plan(&samples, 13).unwrap().unwrap();
    let bits_per_index = 2;
    for (row_index, source_row) in samples.chunks_exact(13).enumerate() {
        let packed_row = &plan.indexed_samples
            [row_index * plan.indexed_width..(row_index + 1) * plan.indexed_width];
        for (column, &sample) in source_row.iter().enumerate() {
            let index = (packed_row[column / 4] >> ((column % 4) * bits_per_index)) & 3;
            assert_eq!(plan.entries[usize::from(index)], sample);
        }
    }
}

#[test]
fn planes_above_the_bounded_palette_use_the_ordinary_path() {
    let samples = (0..=16).collect::<Vec<_>>();
    assert!(make_plan(&samples, samples.len()).unwrap().is_none());
}
