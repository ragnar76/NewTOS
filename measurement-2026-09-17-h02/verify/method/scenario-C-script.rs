    // scenario c (v2): press on the File title (hover already opened the box),
    // hold, drag onto "Open..." (enabled, unlike "Info..." with nothing
    // selected), release there -> selection on RELEASE, no MU_BUTTON.
    const SCRIPT: &[(u32, Step)] = &[
        (1200, Step::Mv(-127, -127)),
        (1201, Step::Mv(-127, -127)),
        (1202, Step::Mv(-127, -127)),
        (1203, Step::Mv(-127, -127)),
        (1204, Step::Mv(-127, -127)),
        (1205, Step::Mv(-127, -127)),
        (1206, Step::Mv(88, 8)),
        (1207, Step::Arm),
        (1400, Step::Mark("C_BOX_OPEN_BEFORE_PRESS")),
        (1600, Step::Dn),
        (1601, Step::Mark("C_PRESSED_ON_TITLE")),
        (1800, Step::MvB(62, 16)),
        (1801, Step::Mark("C_DRAGGED_ON_ITEM_HELD")),
        (2000, Step::Up),
        (2001, Step::Mark("C_RELEASED_ON_ITEM")),
        (2400, Step::Mark("C_SETTLED")),
    ];
