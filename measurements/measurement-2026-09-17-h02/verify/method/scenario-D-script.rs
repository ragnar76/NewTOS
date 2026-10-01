    // scenario d: press on empty desktop, drag INTO the bar over File while
    // still held -- no menu may open until release; hover after release opens it.
    const SCRIPT: &[(u32, Step)] = &[
        (1200, Step::Mv(-127, -127)),
        (1201, Step::Mv(-127, -127)),
        (1202, Step::Mv(-127, -127)),
        (1203, Step::Mv(-127, -127)),
        (1204, Step::Mv(-127, -127)),
        (1205, Step::Mv(-127, -127)),
        (1206, Step::Mv(127, 127)),
        (1207, Step::Mv(127, 127)),
        (1208, Step::Mv(46, 46)),
        (1209, Step::Arm),
        (1400, Step::Mark("D_ON_DESKTOP")),
        (1600, Step::Dn),
        (1601, Step::Mark("D_PRESSED_DESKTOP")),
        (1800, Step::MvB(-127, -127)),
        (1801, Step::MvB(-85, -127)),
        (1802, Step::MvB(0, -38)),
        (2000, Step::Mark("D_HELD_OVER_FILE")),
        (2200, Step::Up),
        (2201, Step::Mark("D_RELEASED")),
        (2400, Step::Mark("D_HOVER_REOPENS")),
    ];
