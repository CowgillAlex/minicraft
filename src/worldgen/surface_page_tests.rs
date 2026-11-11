#[test]
fn span_decoration_matches_literal_pages_and_sampling_order() {
    for (seed, origin) in [
        (1794358123416304200, [-28800, 1024, -32896]),
        (8349847389, [1344, 992, 448]),
        (-67, [2560, -1000, 5760]),
        (1, [2560, -512, 512]),
        (1, [0, 1024, 0]),
    ] {
        let generator = crate::worldgen::Generator::new(seed).unwrap();
        let mut a = generator.sampler();
        let mut b = generator.sampler();
        let preview = crate::worldgen::preview::Preview {
            seed,
            origin: [0, 0, 0],
            ..Default::default()
        };
        for dy in [0, 32, -32] {
            let p = [origin[0], origin[1] + dy, origin[2]];
            let raw = crate::worldgen::page::generate_raw(preview, &mut a, p).data;
            let mut expected = raw.clone();
            let mut actual = raw;
            let mut old = std::mem::take(&mut b.surface_cache);
            old.decorate_literal(b.surface, &mut b, p, &mut expected);
            b.surface_cache = old;
            a.decorate(p, &mut actual);
            assert_eq!(actual.to_vec(), expected.to_vec(), "seed {seed} page {p:?}");
        }
    }
}
