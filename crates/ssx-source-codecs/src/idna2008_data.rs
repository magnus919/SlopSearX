// Generated from pinned IDNA 17.0 literals and CPython/UCD 16.0 static data.

// IDNA literals SHA-256: 01ffa6a3c5819a48402ba4f25ca3901fcf0e5f49903432ad74bed45ad429a699

// UnicodeData 16 SHA-256: ff58e5823bd095166564a006e47d111130813dcf8bf234ef79fa51a870edb48f

// CPython unicodedata_db.h SHA-256: 00a5ed9e602bb55e9f080333ac8cbc360352eea9de960eec69efdb0c3d68b110

// CPython unicodename_db.h SHA-256: b756b8c66e7b73d4e177fa75e88c466322b158c1cbef276d5a4b5dea865c72b5

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Range {
    pub start: u32,
    pub end: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PropertyRange<T> {
    pub start: u32,
    pub end: u32,
    pub value: T,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Decomposition {
    pub point: u32,
    pub offset: usize,
    pub len: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Composition {
    pub first: u32,
    pub second: u32,
    pub result: u32,
}

pub const CLASS_PVALID: &[Range] = &[
    Range {
        start: 0x2D,
        end: 0x2E,
    },
    Range {
        start: 0x30,
        end: 0x3A,
    },
    Range {
        start: 0x61,
        end: 0x7B,
    },
    Range {
        start: 0xDF,
        end: 0xF7,
    },
    Range {
        start: 0xF8,
        end: 0x100,
    },
    Range {
        start: 0x101,
        end: 0x102,
    },
    Range {
        start: 0x103,
        end: 0x104,
    },
    Range {
        start: 0x105,
        end: 0x106,
    },
    Range {
        start: 0x107,
        end: 0x108,
    },
    Range {
        start: 0x109,
        end: 0x10A,
    },
    Range {
        start: 0x10B,
        end: 0x10C,
    },
    Range {
        start: 0x10D,
        end: 0x10E,
    },
    Range {
        start: 0x10F,
        end: 0x110,
    },
    Range {
        start: 0x111,
        end: 0x112,
    },
    Range {
        start: 0x113,
        end: 0x114,
    },
    Range {
        start: 0x115,
        end: 0x116,
    },
    Range {
        start: 0x117,
        end: 0x118,
    },
    Range {
        start: 0x119,
        end: 0x11A,
    },
    Range {
        start: 0x11B,
        end: 0x11C,
    },
    Range {
        start: 0x11D,
        end: 0x11E,
    },
    Range {
        start: 0x11F,
        end: 0x120,
    },
    Range {
        start: 0x121,
        end: 0x122,
    },
    Range {
        start: 0x123,
        end: 0x124,
    },
    Range {
        start: 0x125,
        end: 0x126,
    },
    Range {
        start: 0x127,
        end: 0x128,
    },
    Range {
        start: 0x129,
        end: 0x12A,
    },
    Range {
        start: 0x12B,
        end: 0x12C,
    },
    Range {
        start: 0x12D,
        end: 0x12E,
    },
    Range {
        start: 0x12F,
        end: 0x130,
    },
    Range {
        start: 0x131,
        end: 0x132,
    },
    Range {
        start: 0x135,
        end: 0x136,
    },
    Range {
        start: 0x137,
        end: 0x139,
    },
    Range {
        start: 0x13A,
        end: 0x13B,
    },
    Range {
        start: 0x13C,
        end: 0x13D,
    },
    Range {
        start: 0x13E,
        end: 0x13F,
    },
    Range {
        start: 0x142,
        end: 0x143,
    },
    Range {
        start: 0x144,
        end: 0x145,
    },
    Range {
        start: 0x146,
        end: 0x147,
    },
    Range {
        start: 0x148,
        end: 0x149,
    },
    Range {
        start: 0x14B,
        end: 0x14C,
    },
    Range {
        start: 0x14D,
        end: 0x14E,
    },
    Range {
        start: 0x14F,
        end: 0x150,
    },
    Range {
        start: 0x151,
        end: 0x152,
    },
    Range {
        start: 0x153,
        end: 0x154,
    },
    Range {
        start: 0x155,
        end: 0x156,
    },
    Range {
        start: 0x157,
        end: 0x158,
    },
    Range {
        start: 0x159,
        end: 0x15A,
    },
    Range {
        start: 0x15B,
        end: 0x15C,
    },
    Range {
        start: 0x15D,
        end: 0x15E,
    },
    Range {
        start: 0x15F,
        end: 0x160,
    },
    Range {
        start: 0x161,
        end: 0x162,
    },
    Range {
        start: 0x163,
        end: 0x164,
    },
    Range {
        start: 0x165,
        end: 0x166,
    },
    Range {
        start: 0x167,
        end: 0x168,
    },
    Range {
        start: 0x169,
        end: 0x16A,
    },
    Range {
        start: 0x16B,
        end: 0x16C,
    },
    Range {
        start: 0x16D,
        end: 0x16E,
    },
    Range {
        start: 0x16F,
        end: 0x170,
    },
    Range {
        start: 0x171,
        end: 0x172,
    },
    Range {
        start: 0x173,
        end: 0x174,
    },
    Range {
        start: 0x175,
        end: 0x176,
    },
    Range {
        start: 0x177,
        end: 0x178,
    },
    Range {
        start: 0x17A,
        end: 0x17B,
    },
    Range {
        start: 0x17C,
        end: 0x17D,
    },
    Range {
        start: 0x17E,
        end: 0x17F,
    },
    Range {
        start: 0x180,
        end: 0x181,
    },
    Range {
        start: 0x183,
        end: 0x184,
    },
    Range {
        start: 0x185,
        end: 0x186,
    },
    Range {
        start: 0x188,
        end: 0x189,
    },
    Range {
        start: 0x18C,
        end: 0x18E,
    },
    Range {
        start: 0x192,
        end: 0x193,
    },
    Range {
        start: 0x195,
        end: 0x196,
    },
    Range {
        start: 0x199,
        end: 0x19C,
    },
    Range {
        start: 0x19E,
        end: 0x19F,
    },
    Range {
        start: 0x1A1,
        end: 0x1A2,
    },
    Range {
        start: 0x1A3,
        end: 0x1A4,
    },
    Range {
        start: 0x1A5,
        end: 0x1A6,
    },
    Range {
        start: 0x1A8,
        end: 0x1A9,
    },
    Range {
        start: 0x1AA,
        end: 0x1AC,
    },
    Range {
        start: 0x1AD,
        end: 0x1AE,
    },
    Range {
        start: 0x1B0,
        end: 0x1B1,
    },
    Range {
        start: 0x1B4,
        end: 0x1B5,
    },
    Range {
        start: 0x1B6,
        end: 0x1B7,
    },
    Range {
        start: 0x1B9,
        end: 0x1BC,
    },
    Range {
        start: 0x1BD,
        end: 0x1C4,
    },
    Range {
        start: 0x1CE,
        end: 0x1CF,
    },
    Range {
        start: 0x1D0,
        end: 0x1D1,
    },
    Range {
        start: 0x1D2,
        end: 0x1D3,
    },
    Range {
        start: 0x1D4,
        end: 0x1D5,
    },
    Range {
        start: 0x1D6,
        end: 0x1D7,
    },
    Range {
        start: 0x1D8,
        end: 0x1D9,
    },
    Range {
        start: 0x1DA,
        end: 0x1DB,
    },
    Range {
        start: 0x1DC,
        end: 0x1DE,
    },
    Range {
        start: 0x1DF,
        end: 0x1E0,
    },
    Range {
        start: 0x1E1,
        end: 0x1E2,
    },
    Range {
        start: 0x1E3,
        end: 0x1E4,
    },
    Range {
        start: 0x1E5,
        end: 0x1E6,
    },
    Range {
        start: 0x1E7,
        end: 0x1E8,
    },
    Range {
        start: 0x1E9,
        end: 0x1EA,
    },
    Range {
        start: 0x1EB,
        end: 0x1EC,
    },
    Range {
        start: 0x1ED,
        end: 0x1EE,
    },
    Range {
        start: 0x1EF,
        end: 0x1F1,
    },
    Range {
        start: 0x1F5,
        end: 0x1F6,
    },
    Range {
        start: 0x1F9,
        end: 0x1FA,
    },
    Range {
        start: 0x1FB,
        end: 0x1FC,
    },
    Range {
        start: 0x1FD,
        end: 0x1FE,
    },
    Range {
        start: 0x1FF,
        end: 0x200,
    },
    Range {
        start: 0x201,
        end: 0x202,
    },
    Range {
        start: 0x203,
        end: 0x204,
    },
    Range {
        start: 0x205,
        end: 0x206,
    },
    Range {
        start: 0x207,
        end: 0x208,
    },
    Range {
        start: 0x209,
        end: 0x20A,
    },
    Range {
        start: 0x20B,
        end: 0x20C,
    },
    Range {
        start: 0x20D,
        end: 0x20E,
    },
    Range {
        start: 0x20F,
        end: 0x210,
    },
    Range {
        start: 0x211,
        end: 0x212,
    },
    Range {
        start: 0x213,
        end: 0x214,
    },
    Range {
        start: 0x215,
        end: 0x216,
    },
    Range {
        start: 0x217,
        end: 0x218,
    },
    Range {
        start: 0x219,
        end: 0x21A,
    },
    Range {
        start: 0x21B,
        end: 0x21C,
    },
    Range {
        start: 0x21D,
        end: 0x21E,
    },
    Range {
        start: 0x21F,
        end: 0x220,
    },
    Range {
        start: 0x221,
        end: 0x222,
    },
    Range {
        start: 0x223,
        end: 0x224,
    },
    Range {
        start: 0x225,
        end: 0x226,
    },
    Range {
        start: 0x227,
        end: 0x228,
    },
    Range {
        start: 0x229,
        end: 0x22A,
    },
    Range {
        start: 0x22B,
        end: 0x22C,
    },
    Range {
        start: 0x22D,
        end: 0x22E,
    },
    Range {
        start: 0x22F,
        end: 0x230,
    },
    Range {
        start: 0x231,
        end: 0x232,
    },
    Range {
        start: 0x233,
        end: 0x23A,
    },
    Range {
        start: 0x23C,
        end: 0x23D,
    },
    Range {
        start: 0x23F,
        end: 0x241,
    },
    Range {
        start: 0x242,
        end: 0x243,
    },
    Range {
        start: 0x247,
        end: 0x248,
    },
    Range {
        start: 0x249,
        end: 0x24A,
    },
    Range {
        start: 0x24B,
        end: 0x24C,
    },
    Range {
        start: 0x24D,
        end: 0x24E,
    },
    Range {
        start: 0x24F,
        end: 0x2B0,
    },
    Range {
        start: 0x2B9,
        end: 0x2C2,
    },
    Range {
        start: 0x2C6,
        end: 0x2D2,
    },
    Range {
        start: 0x2EC,
        end: 0x2ED,
    },
    Range {
        start: 0x2EE,
        end: 0x2EF,
    },
    Range {
        start: 0x300,
        end: 0x340,
    },
    Range {
        start: 0x342,
        end: 0x343,
    },
    Range {
        start: 0x346,
        end: 0x34F,
    },
    Range {
        start: 0x350,
        end: 0x370,
    },
    Range {
        start: 0x371,
        end: 0x372,
    },
    Range {
        start: 0x373,
        end: 0x374,
    },
    Range {
        start: 0x377,
        end: 0x378,
    },
    Range {
        start: 0x37B,
        end: 0x37E,
    },
    Range {
        start: 0x390,
        end: 0x391,
    },
    Range {
        start: 0x3AC,
        end: 0x3CF,
    },
    Range {
        start: 0x3D7,
        end: 0x3D8,
    },
    Range {
        start: 0x3D9,
        end: 0x3DA,
    },
    Range {
        start: 0x3DB,
        end: 0x3DC,
    },
    Range {
        start: 0x3DD,
        end: 0x3DE,
    },
    Range {
        start: 0x3DF,
        end: 0x3E0,
    },
    Range {
        start: 0x3E1,
        end: 0x3E2,
    },
    Range {
        start: 0x3E3,
        end: 0x3E4,
    },
    Range {
        start: 0x3E5,
        end: 0x3E6,
    },
    Range {
        start: 0x3E7,
        end: 0x3E8,
    },
    Range {
        start: 0x3E9,
        end: 0x3EA,
    },
    Range {
        start: 0x3EB,
        end: 0x3EC,
    },
    Range {
        start: 0x3ED,
        end: 0x3EE,
    },
    Range {
        start: 0x3EF,
        end: 0x3F0,
    },
    Range {
        start: 0x3F3,
        end: 0x3F4,
    },
    Range {
        start: 0x3F8,
        end: 0x3F9,
    },
    Range {
        start: 0x3FB,
        end: 0x3FD,
    },
    Range {
        start: 0x430,
        end: 0x460,
    },
    Range {
        start: 0x461,
        end: 0x462,
    },
    Range {
        start: 0x463,
        end: 0x464,
    },
    Range {
        start: 0x465,
        end: 0x466,
    },
    Range {
        start: 0x467,
        end: 0x468,
    },
    Range {
        start: 0x469,
        end: 0x46A,
    },
    Range {
        start: 0x46B,
        end: 0x46C,
    },
    Range {
        start: 0x46D,
        end: 0x46E,
    },
    Range {
        start: 0x46F,
        end: 0x470,
    },
    Range {
        start: 0x471,
        end: 0x472,
    },
    Range {
        start: 0x473,
        end: 0x474,
    },
    Range {
        start: 0x475,
        end: 0x476,
    },
    Range {
        start: 0x477,
        end: 0x478,
    },
    Range {
        start: 0x479,
        end: 0x47A,
    },
    Range {
        start: 0x47B,
        end: 0x47C,
    },
    Range {
        start: 0x47D,
        end: 0x47E,
    },
    Range {
        start: 0x47F,
        end: 0x480,
    },
    Range {
        start: 0x481,
        end: 0x482,
    },
    Range {
        start: 0x483,
        end: 0x488,
    },
    Range {
        start: 0x48B,
        end: 0x48C,
    },
    Range {
        start: 0x48D,
        end: 0x48E,
    },
    Range {
        start: 0x48F,
        end: 0x490,
    },
    Range {
        start: 0x491,
        end: 0x492,
    },
    Range {
        start: 0x493,
        end: 0x494,
    },
    Range {
        start: 0x495,
        end: 0x496,
    },
    Range {
        start: 0x497,
        end: 0x498,
    },
    Range {
        start: 0x499,
        end: 0x49A,
    },
    Range {
        start: 0x49B,
        end: 0x49C,
    },
    Range {
        start: 0x49D,
        end: 0x49E,
    },
    Range {
        start: 0x49F,
        end: 0x4A0,
    },
    Range {
        start: 0x4A1,
        end: 0x4A2,
    },
    Range {
        start: 0x4A3,
        end: 0x4A4,
    },
    Range {
        start: 0x4A5,
        end: 0x4A6,
    },
    Range {
        start: 0x4A7,
        end: 0x4A8,
    },
    Range {
        start: 0x4A9,
        end: 0x4AA,
    },
    Range {
        start: 0x4AB,
        end: 0x4AC,
    },
    Range {
        start: 0x4AD,
        end: 0x4AE,
    },
    Range {
        start: 0x4AF,
        end: 0x4B0,
    },
    Range {
        start: 0x4B1,
        end: 0x4B2,
    },
    Range {
        start: 0x4B3,
        end: 0x4B4,
    },
    Range {
        start: 0x4B5,
        end: 0x4B6,
    },
    Range {
        start: 0x4B7,
        end: 0x4B8,
    },
    Range {
        start: 0x4B9,
        end: 0x4BA,
    },
    Range {
        start: 0x4BB,
        end: 0x4BC,
    },
    Range {
        start: 0x4BD,
        end: 0x4BE,
    },
    Range {
        start: 0x4BF,
        end: 0x4C0,
    },
    Range {
        start: 0x4C2,
        end: 0x4C3,
    },
    Range {
        start: 0x4C4,
        end: 0x4C5,
    },
    Range {
        start: 0x4C6,
        end: 0x4C7,
    },
    Range {
        start: 0x4C8,
        end: 0x4C9,
    },
    Range {
        start: 0x4CA,
        end: 0x4CB,
    },
    Range {
        start: 0x4CC,
        end: 0x4CD,
    },
    Range {
        start: 0x4CE,
        end: 0x4D0,
    },
    Range {
        start: 0x4D1,
        end: 0x4D2,
    },
    Range {
        start: 0x4D3,
        end: 0x4D4,
    },
    Range {
        start: 0x4D5,
        end: 0x4D6,
    },
    Range {
        start: 0x4D7,
        end: 0x4D8,
    },
    Range {
        start: 0x4D9,
        end: 0x4DA,
    },
    Range {
        start: 0x4DB,
        end: 0x4DC,
    },
    Range {
        start: 0x4DD,
        end: 0x4DE,
    },
    Range {
        start: 0x4DF,
        end: 0x4E0,
    },
    Range {
        start: 0x4E1,
        end: 0x4E2,
    },
    Range {
        start: 0x4E3,
        end: 0x4E4,
    },
    Range {
        start: 0x4E5,
        end: 0x4E6,
    },
    Range {
        start: 0x4E7,
        end: 0x4E8,
    },
    Range {
        start: 0x4E9,
        end: 0x4EA,
    },
    Range {
        start: 0x4EB,
        end: 0x4EC,
    },
    Range {
        start: 0x4ED,
        end: 0x4EE,
    },
    Range {
        start: 0x4EF,
        end: 0x4F0,
    },
    Range {
        start: 0x4F1,
        end: 0x4F2,
    },
    Range {
        start: 0x4F3,
        end: 0x4F4,
    },
    Range {
        start: 0x4F5,
        end: 0x4F6,
    },
    Range {
        start: 0x4F7,
        end: 0x4F8,
    },
    Range {
        start: 0x4F9,
        end: 0x4FA,
    },
    Range {
        start: 0x4FB,
        end: 0x4FC,
    },
    Range {
        start: 0x4FD,
        end: 0x4FE,
    },
    Range {
        start: 0x4FF,
        end: 0x500,
    },
    Range {
        start: 0x501,
        end: 0x502,
    },
    Range {
        start: 0x503,
        end: 0x504,
    },
    Range {
        start: 0x505,
        end: 0x506,
    },
    Range {
        start: 0x507,
        end: 0x508,
    },
    Range {
        start: 0x509,
        end: 0x50A,
    },
    Range {
        start: 0x50B,
        end: 0x50C,
    },
    Range {
        start: 0x50D,
        end: 0x50E,
    },
    Range {
        start: 0x50F,
        end: 0x510,
    },
    Range {
        start: 0x511,
        end: 0x512,
    },
    Range {
        start: 0x513,
        end: 0x514,
    },
    Range {
        start: 0x515,
        end: 0x516,
    },
    Range {
        start: 0x517,
        end: 0x518,
    },
    Range {
        start: 0x519,
        end: 0x51A,
    },
    Range {
        start: 0x51B,
        end: 0x51C,
    },
    Range {
        start: 0x51D,
        end: 0x51E,
    },
    Range {
        start: 0x51F,
        end: 0x520,
    },
    Range {
        start: 0x521,
        end: 0x522,
    },
    Range {
        start: 0x523,
        end: 0x524,
    },
    Range {
        start: 0x525,
        end: 0x526,
    },
    Range {
        start: 0x527,
        end: 0x528,
    },
    Range {
        start: 0x529,
        end: 0x52A,
    },
    Range {
        start: 0x52B,
        end: 0x52C,
    },
    Range {
        start: 0x52D,
        end: 0x52E,
    },
    Range {
        start: 0x52F,
        end: 0x530,
    },
    Range {
        start: 0x559,
        end: 0x55A,
    },
    Range {
        start: 0x560,
        end: 0x587,
    },
    Range {
        start: 0x588,
        end: 0x589,
    },
    Range {
        start: 0x591,
        end: 0x5BE,
    },
    Range {
        start: 0x5BF,
        end: 0x5C0,
    },
    Range {
        start: 0x5C1,
        end: 0x5C3,
    },
    Range {
        start: 0x5C4,
        end: 0x5C6,
    },
    Range {
        start: 0x5C7,
        end: 0x5C8,
    },
    Range {
        start: 0x5D0,
        end: 0x5EB,
    },
    Range {
        start: 0x5EF,
        end: 0x5F3,
    },
    Range {
        start: 0x610,
        end: 0x61B,
    },
    Range {
        start: 0x620,
        end: 0x640,
    },
    Range {
        start: 0x641,
        end: 0x660,
    },
    Range {
        start: 0x66E,
        end: 0x675,
    },
    Range {
        start: 0x679,
        end: 0x6D4,
    },
    Range {
        start: 0x6D5,
        end: 0x6DD,
    },
    Range {
        start: 0x6DF,
        end: 0x6E9,
    },
    Range {
        start: 0x6EA,
        end: 0x6F0,
    },
    Range {
        start: 0x6FA,
        end: 0x700,
    },
    Range {
        start: 0x710,
        end: 0x74B,
    },
    Range {
        start: 0x74D,
        end: 0x7B2,
    },
    Range {
        start: 0x7C0,
        end: 0x7F6,
    },
    Range {
        start: 0x7FD,
        end: 0x7FE,
    },
    Range {
        start: 0x800,
        end: 0x82E,
    },
    Range {
        start: 0x840,
        end: 0x85C,
    },
    Range {
        start: 0x860,
        end: 0x86B,
    },
    Range {
        start: 0x870,
        end: 0x888,
    },
    Range {
        start: 0x889,
        end: 0x890,
    },
    Range {
        start: 0x897,
        end: 0x8E2,
    },
    Range {
        start: 0x8E3,
        end: 0x958,
    },
    Range {
        start: 0x960,
        end: 0x964,
    },
    Range {
        start: 0x966,
        end: 0x970,
    },
    Range {
        start: 0x971,
        end: 0x984,
    },
    Range {
        start: 0x985,
        end: 0x98D,
    },
    Range {
        start: 0x98F,
        end: 0x991,
    },
    Range {
        start: 0x993,
        end: 0x9A9,
    },
    Range {
        start: 0x9AA,
        end: 0x9B1,
    },
    Range {
        start: 0x9B2,
        end: 0x9B3,
    },
    Range {
        start: 0x9B6,
        end: 0x9BA,
    },
    Range {
        start: 0x9BC,
        end: 0x9C5,
    },
    Range {
        start: 0x9C7,
        end: 0x9C9,
    },
    Range {
        start: 0x9CB,
        end: 0x9CF,
    },
    Range {
        start: 0x9D7,
        end: 0x9D8,
    },
    Range {
        start: 0x9E0,
        end: 0x9E4,
    },
    Range {
        start: 0x9E6,
        end: 0x9F2,
    },
    Range {
        start: 0x9FC,
        end: 0x9FD,
    },
    Range {
        start: 0x9FE,
        end: 0x9FF,
    },
    Range {
        start: 0xA01,
        end: 0xA04,
    },
    Range {
        start: 0xA05,
        end: 0xA0B,
    },
    Range {
        start: 0xA0F,
        end: 0xA11,
    },
    Range {
        start: 0xA13,
        end: 0xA29,
    },
    Range {
        start: 0xA2A,
        end: 0xA31,
    },
    Range {
        start: 0xA32,
        end: 0xA33,
    },
    Range {
        start: 0xA35,
        end: 0xA36,
    },
    Range {
        start: 0xA38,
        end: 0xA3A,
    },
    Range {
        start: 0xA3C,
        end: 0xA3D,
    },
    Range {
        start: 0xA3E,
        end: 0xA43,
    },
    Range {
        start: 0xA47,
        end: 0xA49,
    },
    Range {
        start: 0xA4B,
        end: 0xA4E,
    },
    Range {
        start: 0xA51,
        end: 0xA52,
    },
    Range {
        start: 0xA5C,
        end: 0xA5D,
    },
    Range {
        start: 0xA66,
        end: 0xA76,
    },
    Range {
        start: 0xA81,
        end: 0xA84,
    },
    Range {
        start: 0xA85,
        end: 0xA8E,
    },
    Range {
        start: 0xA8F,
        end: 0xA92,
    },
    Range {
        start: 0xA93,
        end: 0xAA9,
    },
    Range {
        start: 0xAAA,
        end: 0xAB1,
    },
    Range {
        start: 0xAB2,
        end: 0xAB4,
    },
    Range {
        start: 0xAB5,
        end: 0xABA,
    },
    Range {
        start: 0xABC,
        end: 0xAC6,
    },
    Range {
        start: 0xAC7,
        end: 0xACA,
    },
    Range {
        start: 0xACB,
        end: 0xACE,
    },
    Range {
        start: 0xAD0,
        end: 0xAD1,
    },
    Range {
        start: 0xAE0,
        end: 0xAE4,
    },
    Range {
        start: 0xAE6,
        end: 0xAF0,
    },
    Range {
        start: 0xAF9,
        end: 0xB00,
    },
    Range {
        start: 0xB01,
        end: 0xB04,
    },
    Range {
        start: 0xB05,
        end: 0xB0D,
    },
    Range {
        start: 0xB0F,
        end: 0xB11,
    },
    Range {
        start: 0xB13,
        end: 0xB29,
    },
    Range {
        start: 0xB2A,
        end: 0xB31,
    },
    Range {
        start: 0xB32,
        end: 0xB34,
    },
    Range {
        start: 0xB35,
        end: 0xB3A,
    },
    Range {
        start: 0xB3C,
        end: 0xB45,
    },
    Range {
        start: 0xB47,
        end: 0xB49,
    },
    Range {
        start: 0xB4B,
        end: 0xB4E,
    },
    Range {
        start: 0xB55,
        end: 0xB58,
    },
    Range {
        start: 0xB5F,
        end: 0xB64,
    },
    Range {
        start: 0xB66,
        end: 0xB70,
    },
    Range {
        start: 0xB71,
        end: 0xB72,
    },
    Range {
        start: 0xB82,
        end: 0xB84,
    },
    Range {
        start: 0xB85,
        end: 0xB8B,
    },
    Range {
        start: 0xB8E,
        end: 0xB91,
    },
    Range {
        start: 0xB92,
        end: 0xB96,
    },
    Range {
        start: 0xB99,
        end: 0xB9B,
    },
    Range {
        start: 0xB9C,
        end: 0xB9D,
    },
    Range {
        start: 0xB9E,
        end: 0xBA0,
    },
    Range {
        start: 0xBA3,
        end: 0xBA5,
    },
    Range {
        start: 0xBA8,
        end: 0xBAB,
    },
    Range {
        start: 0xBAE,
        end: 0xBBA,
    },
    Range {
        start: 0xBBE,
        end: 0xBC3,
    },
    Range {
        start: 0xBC6,
        end: 0xBC9,
    },
    Range {
        start: 0xBCA,
        end: 0xBCE,
    },
    Range {
        start: 0xBD0,
        end: 0xBD1,
    },
    Range {
        start: 0xBD7,
        end: 0xBD8,
    },
    Range {
        start: 0xBE6,
        end: 0xBF0,
    },
    Range {
        start: 0xC00,
        end: 0xC0D,
    },
    Range {
        start: 0xC0E,
        end: 0xC11,
    },
    Range {
        start: 0xC12,
        end: 0xC29,
    },
    Range {
        start: 0xC2A,
        end: 0xC3A,
    },
    Range {
        start: 0xC3C,
        end: 0xC45,
    },
    Range {
        start: 0xC46,
        end: 0xC49,
    },
    Range {
        start: 0xC4A,
        end: 0xC4E,
    },
    Range {
        start: 0xC55,
        end: 0xC57,
    },
    Range {
        start: 0xC58,
        end: 0xC5B,
    },
    Range {
        start: 0xC5C,
        end: 0xC5E,
    },
    Range {
        start: 0xC60,
        end: 0xC64,
    },
    Range {
        start: 0xC66,
        end: 0xC70,
    },
    Range {
        start: 0xC80,
        end: 0xC84,
    },
    Range {
        start: 0xC85,
        end: 0xC8D,
    },
    Range {
        start: 0xC8E,
        end: 0xC91,
    },
    Range {
        start: 0xC92,
        end: 0xCA9,
    },
    Range {
        start: 0xCAA,
        end: 0xCB4,
    },
    Range {
        start: 0xCB5,
        end: 0xCBA,
    },
    Range {
        start: 0xCBC,
        end: 0xCC5,
    },
    Range {
        start: 0xCC6,
        end: 0xCC9,
    },
    Range {
        start: 0xCCA,
        end: 0xCCE,
    },
    Range {
        start: 0xCD5,
        end: 0xCD7,
    },
    Range {
        start: 0xCDC,
        end: 0xCDF,
    },
    Range {
        start: 0xCE0,
        end: 0xCE4,
    },
    Range {
        start: 0xCE6,
        end: 0xCF0,
    },
    Range {
        start: 0xCF1,
        end: 0xCF4,
    },
    Range {
        start: 0xD00,
        end: 0xD0D,
    },
    Range {
        start: 0xD0E,
        end: 0xD11,
    },
    Range {
        start: 0xD12,
        end: 0xD45,
    },
    Range {
        start: 0xD46,
        end: 0xD49,
    },
    Range {
        start: 0xD4A,
        end: 0xD4F,
    },
    Range {
        start: 0xD54,
        end: 0xD58,
    },
    Range {
        start: 0xD5F,
        end: 0xD64,
    },
    Range {
        start: 0xD66,
        end: 0xD70,
    },
    Range {
        start: 0xD7A,
        end: 0xD80,
    },
    Range {
        start: 0xD81,
        end: 0xD84,
    },
    Range {
        start: 0xD85,
        end: 0xD97,
    },
    Range {
        start: 0xD9A,
        end: 0xDB2,
    },
    Range {
        start: 0xDB3,
        end: 0xDBC,
    },
    Range {
        start: 0xDBD,
        end: 0xDBE,
    },
    Range {
        start: 0xDC0,
        end: 0xDC7,
    },
    Range {
        start: 0xDCA,
        end: 0xDCB,
    },
    Range {
        start: 0xDCF,
        end: 0xDD5,
    },
    Range {
        start: 0xDD6,
        end: 0xDD7,
    },
    Range {
        start: 0xDD8,
        end: 0xDE0,
    },
    Range {
        start: 0xDE6,
        end: 0xDF0,
    },
    Range {
        start: 0xDF2,
        end: 0xDF4,
    },
    Range {
        start: 0xE01,
        end: 0xE33,
    },
    Range {
        start: 0xE34,
        end: 0xE3B,
    },
    Range {
        start: 0xE40,
        end: 0xE4F,
    },
    Range {
        start: 0xE50,
        end: 0xE5A,
    },
    Range {
        start: 0xE81,
        end: 0xE83,
    },
    Range {
        start: 0xE84,
        end: 0xE85,
    },
    Range {
        start: 0xE86,
        end: 0xE8B,
    },
    Range {
        start: 0xE8C,
        end: 0xEA4,
    },
    Range {
        start: 0xEA5,
        end: 0xEA6,
    },
    Range {
        start: 0xEA7,
        end: 0xEB3,
    },
    Range {
        start: 0xEB4,
        end: 0xEBE,
    },
    Range {
        start: 0xEC0,
        end: 0xEC5,
    },
    Range {
        start: 0xEC6,
        end: 0xEC7,
    },
    Range {
        start: 0xEC8,
        end: 0xECF,
    },
    Range {
        start: 0xED0,
        end: 0xEDA,
    },
    Range {
        start: 0xEDE,
        end: 0xEE0,
    },
    Range {
        start: 0xF00,
        end: 0xF01,
    },
    Range {
        start: 0xF0B,
        end: 0xF0C,
    },
    Range {
        start: 0xF18,
        end: 0xF1A,
    },
    Range {
        start: 0xF20,
        end: 0xF2A,
    },
    Range {
        start: 0xF35,
        end: 0xF36,
    },
    Range {
        start: 0xF37,
        end: 0xF38,
    },
    Range {
        start: 0xF39,
        end: 0xF3A,
    },
    Range {
        start: 0xF3E,
        end: 0xF43,
    },
    Range {
        start: 0xF44,
        end: 0xF48,
    },
    Range {
        start: 0xF49,
        end: 0xF4D,
    },
    Range {
        start: 0xF4E,
        end: 0xF52,
    },
    Range {
        start: 0xF53,
        end: 0xF57,
    },
    Range {
        start: 0xF58,
        end: 0xF5C,
    },
    Range {
        start: 0xF5D,
        end: 0xF69,
    },
    Range {
        start: 0xF6A,
        end: 0xF6D,
    },
    Range {
        start: 0xF71,
        end: 0xF73,
    },
    Range {
        start: 0xF74,
        end: 0xF75,
    },
    Range {
        start: 0xF7A,
        end: 0xF81,
    },
    Range {
        start: 0xF82,
        end: 0xF85,
    },
    Range {
        start: 0xF86,
        end: 0xF93,
    },
    Range {
        start: 0xF94,
        end: 0xF98,
    },
    Range {
        start: 0xF99,
        end: 0xF9D,
    },
    Range {
        start: 0xF9E,
        end: 0xFA2,
    },
    Range {
        start: 0xFA3,
        end: 0xFA7,
    },
    Range {
        start: 0xFA8,
        end: 0xFAC,
    },
    Range {
        start: 0xFAD,
        end: 0xFB9,
    },
    Range {
        start: 0xFBA,
        end: 0xFBD,
    },
    Range {
        start: 0xFC6,
        end: 0xFC7,
    },
    Range {
        start: 0x1000,
        end: 0x104A,
    },
    Range {
        start: 0x1050,
        end: 0x109E,
    },
    Range {
        start: 0x10D0,
        end: 0x10FB,
    },
    Range {
        start: 0x10FD,
        end: 0x1100,
    },
    Range {
        start: 0x1200,
        end: 0x1249,
    },
    Range {
        start: 0x124A,
        end: 0x124E,
    },
    Range {
        start: 0x1250,
        end: 0x1257,
    },
    Range {
        start: 0x1258,
        end: 0x1259,
    },
    Range {
        start: 0x125A,
        end: 0x125E,
    },
    Range {
        start: 0x1260,
        end: 0x1289,
    },
    Range {
        start: 0x128A,
        end: 0x128E,
    },
    Range {
        start: 0x1290,
        end: 0x12B1,
    },
    Range {
        start: 0x12B2,
        end: 0x12B6,
    },
    Range {
        start: 0x12B8,
        end: 0x12BF,
    },
    Range {
        start: 0x12C0,
        end: 0x12C1,
    },
    Range {
        start: 0x12C2,
        end: 0x12C6,
    },
    Range {
        start: 0x12C8,
        end: 0x12D7,
    },
    Range {
        start: 0x12D8,
        end: 0x1311,
    },
    Range {
        start: 0x1312,
        end: 0x1316,
    },
    Range {
        start: 0x1318,
        end: 0x135B,
    },
    Range {
        start: 0x135D,
        end: 0x1360,
    },
    Range {
        start: 0x1380,
        end: 0x1390,
    },
    Range {
        start: 0x13A0,
        end: 0x13F6,
    },
    Range {
        start: 0x1401,
        end: 0x166D,
    },
    Range {
        start: 0x166F,
        end: 0x1680,
    },
    Range {
        start: 0x1681,
        end: 0x169B,
    },
    Range {
        start: 0x16A0,
        end: 0x16EB,
    },
    Range {
        start: 0x16F1,
        end: 0x16F9,
    },
    Range {
        start: 0x1700,
        end: 0x1716,
    },
    Range {
        start: 0x171F,
        end: 0x1735,
    },
    Range {
        start: 0x1740,
        end: 0x1754,
    },
    Range {
        start: 0x1760,
        end: 0x176D,
    },
    Range {
        start: 0x176E,
        end: 0x1771,
    },
    Range {
        start: 0x1772,
        end: 0x1774,
    },
    Range {
        start: 0x1780,
        end: 0x17B4,
    },
    Range {
        start: 0x17B6,
        end: 0x17D4,
    },
    Range {
        start: 0x17D7,
        end: 0x17D8,
    },
    Range {
        start: 0x17DC,
        end: 0x17DE,
    },
    Range {
        start: 0x17E0,
        end: 0x17EA,
    },
    Range {
        start: 0x1810,
        end: 0x181A,
    },
    Range {
        start: 0x1820,
        end: 0x1879,
    },
    Range {
        start: 0x1880,
        end: 0x18AB,
    },
    Range {
        start: 0x18B0,
        end: 0x18F6,
    },
    Range {
        start: 0x1900,
        end: 0x191F,
    },
    Range {
        start: 0x1920,
        end: 0x192C,
    },
    Range {
        start: 0x1930,
        end: 0x193C,
    },
    Range {
        start: 0x1946,
        end: 0x196E,
    },
    Range {
        start: 0x1970,
        end: 0x1975,
    },
    Range {
        start: 0x1980,
        end: 0x19AC,
    },
    Range {
        start: 0x19B0,
        end: 0x19CA,
    },
    Range {
        start: 0x19D0,
        end: 0x19DA,
    },
    Range {
        start: 0x1A00,
        end: 0x1A1C,
    },
    Range {
        start: 0x1A20,
        end: 0x1A5F,
    },
    Range {
        start: 0x1A60,
        end: 0x1A7D,
    },
    Range {
        start: 0x1A7F,
        end: 0x1A8A,
    },
    Range {
        start: 0x1A90,
        end: 0x1A9A,
    },
    Range {
        start: 0x1AA7,
        end: 0x1AA8,
    },
    Range {
        start: 0x1AB0,
        end: 0x1ABE,
    },
    Range {
        start: 0x1ABF,
        end: 0x1ADE,
    },
    Range {
        start: 0x1AE0,
        end: 0x1AEC,
    },
    Range {
        start: 0x1B00,
        end: 0x1B4D,
    },
    Range {
        start: 0x1B50,
        end: 0x1B5A,
    },
    Range {
        start: 0x1B6B,
        end: 0x1B74,
    },
    Range {
        start: 0x1B80,
        end: 0x1BF4,
    },
    Range {
        start: 0x1C00,
        end: 0x1C38,
    },
    Range {
        start: 0x1C40,
        end: 0x1C4A,
    },
    Range {
        start: 0x1C4D,
        end: 0x1C7E,
    },
    Range {
        start: 0x1C8A,
        end: 0x1C8B,
    },
    Range {
        start: 0x1CD0,
        end: 0x1CD3,
    },
    Range {
        start: 0x1CD4,
        end: 0x1CFB,
    },
    Range {
        start: 0x1D00,
        end: 0x1D2C,
    },
    Range {
        start: 0x1D2F,
        end: 0x1D30,
    },
    Range {
        start: 0x1D3B,
        end: 0x1D3C,
    },
    Range {
        start: 0x1D4E,
        end: 0x1D4F,
    },
    Range {
        start: 0x1D6B,
        end: 0x1D78,
    },
    Range {
        start: 0x1D79,
        end: 0x1D9B,
    },
    Range {
        start: 0x1DC0,
        end: 0x1E00,
    },
    Range {
        start: 0x1E01,
        end: 0x1E02,
    },
    Range {
        start: 0x1E03,
        end: 0x1E04,
    },
    Range {
        start: 0x1E05,
        end: 0x1E06,
    },
    Range {
        start: 0x1E07,
        end: 0x1E08,
    },
    Range {
        start: 0x1E09,
        end: 0x1E0A,
    },
    Range {
        start: 0x1E0B,
        end: 0x1E0C,
    },
    Range {
        start: 0x1E0D,
        end: 0x1E0E,
    },
    Range {
        start: 0x1E0F,
        end: 0x1E10,
    },
    Range {
        start: 0x1E11,
        end: 0x1E12,
    },
    Range {
        start: 0x1E13,
        end: 0x1E14,
    },
    Range {
        start: 0x1E15,
        end: 0x1E16,
    },
    Range {
        start: 0x1E17,
        end: 0x1E18,
    },
    Range {
        start: 0x1E19,
        end: 0x1E1A,
    },
    Range {
        start: 0x1E1B,
        end: 0x1E1C,
    },
    Range {
        start: 0x1E1D,
        end: 0x1E1E,
    },
    Range {
        start: 0x1E1F,
        end: 0x1E20,
    },
    Range {
        start: 0x1E21,
        end: 0x1E22,
    },
    Range {
        start: 0x1E23,
        end: 0x1E24,
    },
    Range {
        start: 0x1E25,
        end: 0x1E26,
    },
    Range {
        start: 0x1E27,
        end: 0x1E28,
    },
    Range {
        start: 0x1E29,
        end: 0x1E2A,
    },
    Range {
        start: 0x1E2B,
        end: 0x1E2C,
    },
    Range {
        start: 0x1E2D,
        end: 0x1E2E,
    },
    Range {
        start: 0x1E2F,
        end: 0x1E30,
    },
    Range {
        start: 0x1E31,
        end: 0x1E32,
    },
    Range {
        start: 0x1E33,
        end: 0x1E34,
    },
    Range {
        start: 0x1E35,
        end: 0x1E36,
    },
    Range {
        start: 0x1E37,
        end: 0x1E38,
    },
    Range {
        start: 0x1E39,
        end: 0x1E3A,
    },
    Range {
        start: 0x1E3B,
        end: 0x1E3C,
    },
    Range {
        start: 0x1E3D,
        end: 0x1E3E,
    },
    Range {
        start: 0x1E3F,
        end: 0x1E40,
    },
    Range {
        start: 0x1E41,
        end: 0x1E42,
    },
    Range {
        start: 0x1E43,
        end: 0x1E44,
    },
    Range {
        start: 0x1E45,
        end: 0x1E46,
    },
    Range {
        start: 0x1E47,
        end: 0x1E48,
    },
    Range {
        start: 0x1E49,
        end: 0x1E4A,
    },
    Range {
        start: 0x1E4B,
        end: 0x1E4C,
    },
    Range {
        start: 0x1E4D,
        end: 0x1E4E,
    },
    Range {
        start: 0x1E4F,
        end: 0x1E50,
    },
    Range {
        start: 0x1E51,
        end: 0x1E52,
    },
    Range {
        start: 0x1E53,
        end: 0x1E54,
    },
    Range {
        start: 0x1E55,
        end: 0x1E56,
    },
    Range {
        start: 0x1E57,
        end: 0x1E58,
    },
    Range {
        start: 0x1E59,
        end: 0x1E5A,
    },
    Range {
        start: 0x1E5B,
        end: 0x1E5C,
    },
    Range {
        start: 0x1E5D,
        end: 0x1E5E,
    },
    Range {
        start: 0x1E5F,
        end: 0x1E60,
    },
    Range {
        start: 0x1E61,
        end: 0x1E62,
    },
    Range {
        start: 0x1E63,
        end: 0x1E64,
    },
    Range {
        start: 0x1E65,
        end: 0x1E66,
    },
    Range {
        start: 0x1E67,
        end: 0x1E68,
    },
    Range {
        start: 0x1E69,
        end: 0x1E6A,
    },
    Range {
        start: 0x1E6B,
        end: 0x1E6C,
    },
    Range {
        start: 0x1E6D,
        end: 0x1E6E,
    },
    Range {
        start: 0x1E6F,
        end: 0x1E70,
    },
    Range {
        start: 0x1E71,
        end: 0x1E72,
    },
    Range {
        start: 0x1E73,
        end: 0x1E74,
    },
    Range {
        start: 0x1E75,
        end: 0x1E76,
    },
    Range {
        start: 0x1E77,
        end: 0x1E78,
    },
    Range {
        start: 0x1E79,
        end: 0x1E7A,
    },
    Range {
        start: 0x1E7B,
        end: 0x1E7C,
    },
    Range {
        start: 0x1E7D,
        end: 0x1E7E,
    },
    Range {
        start: 0x1E7F,
        end: 0x1E80,
    },
    Range {
        start: 0x1E81,
        end: 0x1E82,
    },
    Range {
        start: 0x1E83,
        end: 0x1E84,
    },
    Range {
        start: 0x1E85,
        end: 0x1E86,
    },
    Range {
        start: 0x1E87,
        end: 0x1E88,
    },
    Range {
        start: 0x1E89,
        end: 0x1E8A,
    },
    Range {
        start: 0x1E8B,
        end: 0x1E8C,
    },
    Range {
        start: 0x1E8D,
        end: 0x1E8E,
    },
    Range {
        start: 0x1E8F,
        end: 0x1E90,
    },
    Range {
        start: 0x1E91,
        end: 0x1E92,
    },
    Range {
        start: 0x1E93,
        end: 0x1E94,
    },
    Range {
        start: 0x1E95,
        end: 0x1E9A,
    },
    Range {
        start: 0x1E9C,
        end: 0x1E9E,
    },
    Range {
        start: 0x1E9F,
        end: 0x1EA0,
    },
    Range {
        start: 0x1EA1,
        end: 0x1EA2,
    },
    Range {
        start: 0x1EA3,
        end: 0x1EA4,
    },
    Range {
        start: 0x1EA5,
        end: 0x1EA6,
    },
    Range {
        start: 0x1EA7,
        end: 0x1EA8,
    },
    Range {
        start: 0x1EA9,
        end: 0x1EAA,
    },
    Range {
        start: 0x1EAB,
        end: 0x1EAC,
    },
    Range {
        start: 0x1EAD,
        end: 0x1EAE,
    },
    Range {
        start: 0x1EAF,
        end: 0x1EB0,
    },
    Range {
        start: 0x1EB1,
        end: 0x1EB2,
    },
    Range {
        start: 0x1EB3,
        end: 0x1EB4,
    },
    Range {
        start: 0x1EB5,
        end: 0x1EB6,
    },
    Range {
        start: 0x1EB7,
        end: 0x1EB8,
    },
    Range {
        start: 0x1EB9,
        end: 0x1EBA,
    },
    Range {
        start: 0x1EBB,
        end: 0x1EBC,
    },
    Range {
        start: 0x1EBD,
        end: 0x1EBE,
    },
    Range {
        start: 0x1EBF,
        end: 0x1EC0,
    },
    Range {
        start: 0x1EC1,
        end: 0x1EC2,
    },
    Range {
        start: 0x1EC3,
        end: 0x1EC4,
    },
    Range {
        start: 0x1EC5,
        end: 0x1EC6,
    },
    Range {
        start: 0x1EC7,
        end: 0x1EC8,
    },
    Range {
        start: 0x1EC9,
        end: 0x1ECA,
    },
    Range {
        start: 0x1ECB,
        end: 0x1ECC,
    },
    Range {
        start: 0x1ECD,
        end: 0x1ECE,
    },
    Range {
        start: 0x1ECF,
        end: 0x1ED0,
    },
    Range {
        start: 0x1ED1,
        end: 0x1ED2,
    },
    Range {
        start: 0x1ED3,
        end: 0x1ED4,
    },
    Range {
        start: 0x1ED5,
        end: 0x1ED6,
    },
    Range {
        start: 0x1ED7,
        end: 0x1ED8,
    },
    Range {
        start: 0x1ED9,
        end: 0x1EDA,
    },
    Range {
        start: 0x1EDB,
        end: 0x1EDC,
    },
    Range {
        start: 0x1EDD,
        end: 0x1EDE,
    },
    Range {
        start: 0x1EDF,
        end: 0x1EE0,
    },
    Range {
        start: 0x1EE1,
        end: 0x1EE2,
    },
    Range {
        start: 0x1EE3,
        end: 0x1EE4,
    },
    Range {
        start: 0x1EE5,
        end: 0x1EE6,
    },
    Range {
        start: 0x1EE7,
        end: 0x1EE8,
    },
    Range {
        start: 0x1EE9,
        end: 0x1EEA,
    },
    Range {
        start: 0x1EEB,
        end: 0x1EEC,
    },
    Range {
        start: 0x1EED,
        end: 0x1EEE,
    },
    Range {
        start: 0x1EEF,
        end: 0x1EF0,
    },
    Range {
        start: 0x1EF1,
        end: 0x1EF2,
    },
    Range {
        start: 0x1EF3,
        end: 0x1EF4,
    },
    Range {
        start: 0x1EF5,
        end: 0x1EF6,
    },
    Range {
        start: 0x1EF7,
        end: 0x1EF8,
    },
    Range {
        start: 0x1EF9,
        end: 0x1EFA,
    },
    Range {
        start: 0x1EFB,
        end: 0x1EFC,
    },
    Range {
        start: 0x1EFD,
        end: 0x1EFE,
    },
    Range {
        start: 0x1EFF,
        end: 0x1F08,
    },
    Range {
        start: 0x1F10,
        end: 0x1F16,
    },
    Range {
        start: 0x1F20,
        end: 0x1F28,
    },
    Range {
        start: 0x1F30,
        end: 0x1F38,
    },
    Range {
        start: 0x1F40,
        end: 0x1F46,
    },
    Range {
        start: 0x1F50,
        end: 0x1F58,
    },
    Range {
        start: 0x1F60,
        end: 0x1F68,
    },
    Range {
        start: 0x1F70,
        end: 0x1F71,
    },
    Range {
        start: 0x1F72,
        end: 0x1F73,
    },
    Range {
        start: 0x1F74,
        end: 0x1F75,
    },
    Range {
        start: 0x1F76,
        end: 0x1F77,
    },
    Range {
        start: 0x1F78,
        end: 0x1F79,
    },
    Range {
        start: 0x1F7A,
        end: 0x1F7B,
    },
    Range {
        start: 0x1F7C,
        end: 0x1F7D,
    },
    Range {
        start: 0x1FB0,
        end: 0x1FB2,
    },
    Range {
        start: 0x1FB6,
        end: 0x1FB7,
    },
    Range {
        start: 0x1FC6,
        end: 0x1FC7,
    },
    Range {
        start: 0x1FD0,
        end: 0x1FD3,
    },
    Range {
        start: 0x1FD6,
        end: 0x1FD8,
    },
    Range {
        start: 0x1FE0,
        end: 0x1FE3,
    },
    Range {
        start: 0x1FE4,
        end: 0x1FE8,
    },
    Range {
        start: 0x1FF6,
        end: 0x1FF7,
    },
    Range {
        start: 0x214E,
        end: 0x214F,
    },
    Range {
        start: 0x2184,
        end: 0x2185,
    },
    Range {
        start: 0x2C30,
        end: 0x2C60,
    },
    Range {
        start: 0x2C61,
        end: 0x2C62,
    },
    Range {
        start: 0x2C65,
        end: 0x2C67,
    },
    Range {
        start: 0x2C68,
        end: 0x2C69,
    },
    Range {
        start: 0x2C6A,
        end: 0x2C6B,
    },
    Range {
        start: 0x2C6C,
        end: 0x2C6D,
    },
    Range {
        start: 0x2C71,
        end: 0x2C72,
    },
    Range {
        start: 0x2C73,
        end: 0x2C75,
    },
    Range {
        start: 0x2C76,
        end: 0x2C7C,
    },
    Range {
        start: 0x2C81,
        end: 0x2C82,
    },
    Range {
        start: 0x2C83,
        end: 0x2C84,
    },
    Range {
        start: 0x2C85,
        end: 0x2C86,
    },
    Range {
        start: 0x2C87,
        end: 0x2C88,
    },
    Range {
        start: 0x2C89,
        end: 0x2C8A,
    },
    Range {
        start: 0x2C8B,
        end: 0x2C8C,
    },
    Range {
        start: 0x2C8D,
        end: 0x2C8E,
    },
    Range {
        start: 0x2C8F,
        end: 0x2C90,
    },
    Range {
        start: 0x2C91,
        end: 0x2C92,
    },
    Range {
        start: 0x2C93,
        end: 0x2C94,
    },
    Range {
        start: 0x2C95,
        end: 0x2C96,
    },
    Range {
        start: 0x2C97,
        end: 0x2C98,
    },
    Range {
        start: 0x2C99,
        end: 0x2C9A,
    },
    Range {
        start: 0x2C9B,
        end: 0x2C9C,
    },
    Range {
        start: 0x2C9D,
        end: 0x2C9E,
    },
    Range {
        start: 0x2C9F,
        end: 0x2CA0,
    },
    Range {
        start: 0x2CA1,
        end: 0x2CA2,
    },
    Range {
        start: 0x2CA3,
        end: 0x2CA4,
    },
    Range {
        start: 0x2CA5,
        end: 0x2CA6,
    },
    Range {
        start: 0x2CA7,
        end: 0x2CA8,
    },
    Range {
        start: 0x2CA9,
        end: 0x2CAA,
    },
    Range {
        start: 0x2CAB,
        end: 0x2CAC,
    },
    Range {
        start: 0x2CAD,
        end: 0x2CAE,
    },
    Range {
        start: 0x2CAF,
        end: 0x2CB0,
    },
    Range {
        start: 0x2CB1,
        end: 0x2CB2,
    },
    Range {
        start: 0x2CB3,
        end: 0x2CB4,
    },
    Range {
        start: 0x2CB5,
        end: 0x2CB6,
    },
    Range {
        start: 0x2CB7,
        end: 0x2CB8,
    },
    Range {
        start: 0x2CB9,
        end: 0x2CBA,
    },
    Range {
        start: 0x2CBB,
        end: 0x2CBC,
    },
    Range {
        start: 0x2CBD,
        end: 0x2CBE,
    },
    Range {
        start: 0x2CBF,
        end: 0x2CC0,
    },
    Range {
        start: 0x2CC1,
        end: 0x2CC2,
    },
    Range {
        start: 0x2CC3,
        end: 0x2CC4,
    },
    Range {
        start: 0x2CC5,
        end: 0x2CC6,
    },
    Range {
        start: 0x2CC7,
        end: 0x2CC8,
    },
    Range {
        start: 0x2CC9,
        end: 0x2CCA,
    },
    Range {
        start: 0x2CCB,
        end: 0x2CCC,
    },
    Range {
        start: 0x2CCD,
        end: 0x2CCE,
    },
    Range {
        start: 0x2CCF,
        end: 0x2CD0,
    },
    Range {
        start: 0x2CD1,
        end: 0x2CD2,
    },
    Range {
        start: 0x2CD3,
        end: 0x2CD4,
    },
    Range {
        start: 0x2CD5,
        end: 0x2CD6,
    },
    Range {
        start: 0x2CD7,
        end: 0x2CD8,
    },
    Range {
        start: 0x2CD9,
        end: 0x2CDA,
    },
    Range {
        start: 0x2CDB,
        end: 0x2CDC,
    },
    Range {
        start: 0x2CDD,
        end: 0x2CDE,
    },
    Range {
        start: 0x2CDF,
        end: 0x2CE0,
    },
    Range {
        start: 0x2CE1,
        end: 0x2CE2,
    },
    Range {
        start: 0x2CE3,
        end: 0x2CE5,
    },
    Range {
        start: 0x2CEC,
        end: 0x2CED,
    },
    Range {
        start: 0x2CEE,
        end: 0x2CF2,
    },
    Range {
        start: 0x2CF3,
        end: 0x2CF4,
    },
    Range {
        start: 0x2D00,
        end: 0x2D26,
    },
    Range {
        start: 0x2D27,
        end: 0x2D28,
    },
    Range {
        start: 0x2D2D,
        end: 0x2D2E,
    },
    Range {
        start: 0x2D30,
        end: 0x2D68,
    },
    Range {
        start: 0x2D7F,
        end: 0x2D97,
    },
    Range {
        start: 0x2DA0,
        end: 0x2DA7,
    },
    Range {
        start: 0x2DA8,
        end: 0x2DAF,
    },
    Range {
        start: 0x2DB0,
        end: 0x2DB7,
    },
    Range {
        start: 0x2DB8,
        end: 0x2DBF,
    },
    Range {
        start: 0x2DC0,
        end: 0x2DC7,
    },
    Range {
        start: 0x2DC8,
        end: 0x2DCF,
    },
    Range {
        start: 0x2DD0,
        end: 0x2DD7,
    },
    Range {
        start: 0x2DD8,
        end: 0x2DDF,
    },
    Range {
        start: 0x2DE0,
        end: 0x2E00,
    },
    Range {
        start: 0x2E2F,
        end: 0x2E30,
    },
    Range {
        start: 0x3005,
        end: 0x3008,
    },
    Range {
        start: 0x302A,
        end: 0x302E,
    },
    Range {
        start: 0x303C,
        end: 0x303D,
    },
    Range {
        start: 0x3041,
        end: 0x3097,
    },
    Range {
        start: 0x3099,
        end: 0x309B,
    },
    Range {
        start: 0x309D,
        end: 0x309F,
    },
    Range {
        start: 0x30A1,
        end: 0x30FB,
    },
    Range {
        start: 0x30FC,
        end: 0x30FF,
    },
    Range {
        start: 0x3105,
        end: 0x3130,
    },
    Range {
        start: 0x31A0,
        end: 0x31C0,
    },
    Range {
        start: 0x31F0,
        end: 0x3200,
    },
    Range {
        start: 0x3400,
        end: 0x4DC0,
    },
    Range {
        start: 0x4E00,
        end: 0xA48D,
    },
    Range {
        start: 0xA4D0,
        end: 0xA4FE,
    },
    Range {
        start: 0xA500,
        end: 0xA60D,
    },
    Range {
        start: 0xA610,
        end: 0xA62C,
    },
    Range {
        start: 0xA641,
        end: 0xA642,
    },
    Range {
        start: 0xA643,
        end: 0xA644,
    },
    Range {
        start: 0xA645,
        end: 0xA646,
    },
    Range {
        start: 0xA647,
        end: 0xA648,
    },
    Range {
        start: 0xA649,
        end: 0xA64A,
    },
    Range {
        start: 0xA64B,
        end: 0xA64C,
    },
    Range {
        start: 0xA64D,
        end: 0xA64E,
    },
    Range {
        start: 0xA64F,
        end: 0xA650,
    },
    Range {
        start: 0xA651,
        end: 0xA652,
    },
    Range {
        start: 0xA653,
        end: 0xA654,
    },
    Range {
        start: 0xA655,
        end: 0xA656,
    },
    Range {
        start: 0xA657,
        end: 0xA658,
    },
    Range {
        start: 0xA659,
        end: 0xA65A,
    },
    Range {
        start: 0xA65B,
        end: 0xA65C,
    },
    Range {
        start: 0xA65D,
        end: 0xA65E,
    },
    Range {
        start: 0xA65F,
        end: 0xA660,
    },
    Range {
        start: 0xA661,
        end: 0xA662,
    },
    Range {
        start: 0xA663,
        end: 0xA664,
    },
    Range {
        start: 0xA665,
        end: 0xA666,
    },
    Range {
        start: 0xA667,
        end: 0xA668,
    },
    Range {
        start: 0xA669,
        end: 0xA66A,
    },
    Range {
        start: 0xA66B,
        end: 0xA66C,
    },
    Range {
        start: 0xA66D,
        end: 0xA670,
    },
    Range {
        start: 0xA674,
        end: 0xA67E,
    },
    Range {
        start: 0xA67F,
        end: 0xA680,
    },
    Range {
        start: 0xA681,
        end: 0xA682,
    },
    Range {
        start: 0xA683,
        end: 0xA684,
    },
    Range {
        start: 0xA685,
        end: 0xA686,
    },
    Range {
        start: 0xA687,
        end: 0xA688,
    },
    Range {
        start: 0xA689,
        end: 0xA68A,
    },
    Range {
        start: 0xA68B,
        end: 0xA68C,
    },
    Range {
        start: 0xA68D,
        end: 0xA68E,
    },
    Range {
        start: 0xA68F,
        end: 0xA690,
    },
    Range {
        start: 0xA691,
        end: 0xA692,
    },
    Range {
        start: 0xA693,
        end: 0xA694,
    },
    Range {
        start: 0xA695,
        end: 0xA696,
    },
    Range {
        start: 0xA697,
        end: 0xA698,
    },
    Range {
        start: 0xA699,
        end: 0xA69A,
    },
    Range {
        start: 0xA69B,
        end: 0xA69C,
    },
    Range {
        start: 0xA69E,
        end: 0xA6E6,
    },
    Range {
        start: 0xA6F0,
        end: 0xA6F2,
    },
    Range {
        start: 0xA717,
        end: 0xA720,
    },
    Range {
        start: 0xA723,
        end: 0xA724,
    },
    Range {
        start: 0xA725,
        end: 0xA726,
    },
    Range {
        start: 0xA727,
        end: 0xA728,
    },
    Range {
        start: 0xA729,
        end: 0xA72A,
    },
    Range {
        start: 0xA72B,
        end: 0xA72C,
    },
    Range {
        start: 0xA72D,
        end: 0xA72E,
    },
    Range {
        start: 0xA72F,
        end: 0xA732,
    },
    Range {
        start: 0xA733,
        end: 0xA734,
    },
    Range {
        start: 0xA735,
        end: 0xA736,
    },
    Range {
        start: 0xA737,
        end: 0xA738,
    },
    Range {
        start: 0xA739,
        end: 0xA73A,
    },
    Range {
        start: 0xA73B,
        end: 0xA73C,
    },
    Range {
        start: 0xA73D,
        end: 0xA73E,
    },
    Range {
        start: 0xA73F,
        end: 0xA740,
    },
    Range {
        start: 0xA741,
        end: 0xA742,
    },
    Range {
        start: 0xA743,
        end: 0xA744,
    },
    Range {
        start: 0xA745,
        end: 0xA746,
    },
    Range {
        start: 0xA747,
        end: 0xA748,
    },
    Range {
        start: 0xA749,
        end: 0xA74A,
    },
    Range {
        start: 0xA74B,
        end: 0xA74C,
    },
    Range {
        start: 0xA74D,
        end: 0xA74E,
    },
    Range {
        start: 0xA74F,
        end: 0xA750,
    },
    Range {
        start: 0xA751,
        end: 0xA752,
    },
    Range {
        start: 0xA753,
        end: 0xA754,
    },
    Range {
        start: 0xA755,
        end: 0xA756,
    },
    Range {
        start: 0xA757,
        end: 0xA758,
    },
    Range {
        start: 0xA759,
        end: 0xA75A,
    },
    Range {
        start: 0xA75B,
        end: 0xA75C,
    },
    Range {
        start: 0xA75D,
        end: 0xA75E,
    },
    Range {
        start: 0xA75F,
        end: 0xA760,
    },
    Range {
        start: 0xA761,
        end: 0xA762,
    },
    Range {
        start: 0xA763,
        end: 0xA764,
    },
    Range {
        start: 0xA765,
        end: 0xA766,
    },
    Range {
        start: 0xA767,
        end: 0xA768,
    },
    Range {
        start: 0xA769,
        end: 0xA76A,
    },
    Range {
        start: 0xA76B,
        end: 0xA76C,
    },
    Range {
        start: 0xA76D,
        end: 0xA76E,
    },
    Range {
        start: 0xA76F,
        end: 0xA770,
    },
    Range {
        start: 0xA771,
        end: 0xA779,
    },
    Range {
        start: 0xA77A,
        end: 0xA77B,
    },
    Range {
        start: 0xA77C,
        end: 0xA77D,
    },
    Range {
        start: 0xA77F,
        end: 0xA780,
    },
    Range {
        start: 0xA781,
        end: 0xA782,
    },
    Range {
        start: 0xA783,
        end: 0xA784,
    },
    Range {
        start: 0xA785,
        end: 0xA786,
    },
    Range {
        start: 0xA787,
        end: 0xA789,
    },
    Range {
        start: 0xA78C,
        end: 0xA78D,
    },
    Range {
        start: 0xA78E,
        end: 0xA790,
    },
    Range {
        start: 0xA791,
        end: 0xA792,
    },
    Range {
        start: 0xA793,
        end: 0xA796,
    },
    Range {
        start: 0xA797,
        end: 0xA798,
    },
    Range {
        start: 0xA799,
        end: 0xA79A,
    },
    Range {
        start: 0xA79B,
        end: 0xA79C,
    },
    Range {
        start: 0xA79D,
        end: 0xA79E,
    },
    Range {
        start: 0xA79F,
        end: 0xA7A0,
    },
    Range {
        start: 0xA7A1,
        end: 0xA7A2,
    },
    Range {
        start: 0xA7A3,
        end: 0xA7A4,
    },
    Range {
        start: 0xA7A5,
        end: 0xA7A6,
    },
    Range {
        start: 0xA7A7,
        end: 0xA7A8,
    },
    Range {
        start: 0xA7A9,
        end: 0xA7AA,
    },
    Range {
        start: 0xA7AF,
        end: 0xA7B0,
    },
    Range {
        start: 0xA7B5,
        end: 0xA7B6,
    },
    Range {
        start: 0xA7B7,
        end: 0xA7B8,
    },
    Range {
        start: 0xA7B9,
        end: 0xA7BA,
    },
    Range {
        start: 0xA7BB,
        end: 0xA7BC,
    },
    Range {
        start: 0xA7BD,
        end: 0xA7BE,
    },
    Range {
        start: 0xA7BF,
        end: 0xA7C0,
    },
    Range {
        start: 0xA7C1,
        end: 0xA7C2,
    },
    Range {
        start: 0xA7C3,
        end: 0xA7C4,
    },
    Range {
        start: 0xA7C8,
        end: 0xA7C9,
    },
    Range {
        start: 0xA7CA,
        end: 0xA7CB,
    },
    Range {
        start: 0xA7CD,
        end: 0xA7CE,
    },
    Range {
        start: 0xA7CF,
        end: 0xA7D0,
    },
    Range {
        start: 0xA7D1,
        end: 0xA7D2,
    },
    Range {
        start: 0xA7D3,
        end: 0xA7D4,
    },
    Range {
        start: 0xA7D5,
        end: 0xA7D6,
    },
    Range {
        start: 0xA7D7,
        end: 0xA7D8,
    },
    Range {
        start: 0xA7D9,
        end: 0xA7DA,
    },
    Range {
        start: 0xA7DB,
        end: 0xA7DC,
    },
    Range {
        start: 0xA7F6,
        end: 0xA7F8,
    },
    Range {
        start: 0xA7FA,
        end: 0xA828,
    },
    Range {
        start: 0xA82C,
        end: 0xA82D,
    },
    Range {
        start: 0xA840,
        end: 0xA874,
    },
    Range {
        start: 0xA880,
        end: 0xA8C6,
    },
    Range {
        start: 0xA8D0,
        end: 0xA8DA,
    },
    Range {
        start: 0xA8E0,
        end: 0xA8F8,
    },
    Range {
        start: 0xA8FB,
        end: 0xA8FC,
    },
    Range {
        start: 0xA8FD,
        end: 0xA92E,
    },
    Range {
        start: 0xA930,
        end: 0xA954,
    },
    Range {
        start: 0xA980,
        end: 0xA9C1,
    },
    Range {
        start: 0xA9CF,
        end: 0xA9DA,
    },
    Range {
        start: 0xA9E0,
        end: 0xA9FF,
    },
    Range {
        start: 0xAA00,
        end: 0xAA37,
    },
    Range {
        start: 0xAA40,
        end: 0xAA4E,
    },
    Range {
        start: 0xAA50,
        end: 0xAA5A,
    },
    Range {
        start: 0xAA60,
        end: 0xAA77,
    },
    Range {
        start: 0xAA7A,
        end: 0xAAC3,
    },
    Range {
        start: 0xAADB,
        end: 0xAADE,
    },
    Range {
        start: 0xAAE0,
        end: 0xAAF0,
    },
    Range {
        start: 0xAAF2,
        end: 0xAAF7,
    },
    Range {
        start: 0xAB01,
        end: 0xAB07,
    },
    Range {
        start: 0xAB09,
        end: 0xAB0F,
    },
    Range {
        start: 0xAB11,
        end: 0xAB17,
    },
    Range {
        start: 0xAB20,
        end: 0xAB27,
    },
    Range {
        start: 0xAB28,
        end: 0xAB2F,
    },
    Range {
        start: 0xAB30,
        end: 0xAB5B,
    },
    Range {
        start: 0xAB60,
        end: 0xAB69,
    },
    Range {
        start: 0xABC0,
        end: 0xABEB,
    },
    Range {
        start: 0xABEC,
        end: 0xABEE,
    },
    Range {
        start: 0xABF0,
        end: 0xABFA,
    },
    Range {
        start: 0xAC00,
        end: 0xD7A4,
    },
    Range {
        start: 0xFA0E,
        end: 0xFA10,
    },
    Range {
        start: 0xFA11,
        end: 0xFA12,
    },
    Range {
        start: 0xFA13,
        end: 0xFA15,
    },
    Range {
        start: 0xFA1F,
        end: 0xFA20,
    },
    Range {
        start: 0xFA21,
        end: 0xFA22,
    },
    Range {
        start: 0xFA23,
        end: 0xFA25,
    },
    Range {
        start: 0xFA27,
        end: 0xFA2A,
    },
    Range {
        start: 0xFB1E,
        end: 0xFB1F,
    },
    Range {
        start: 0xFE20,
        end: 0xFE30,
    },
    Range {
        start: 0xFE73,
        end: 0xFE74,
    },
    Range {
        start: 0x10000,
        end: 0x1000C,
    },
    Range {
        start: 0x1000D,
        end: 0x10027,
    },
    Range {
        start: 0x10028,
        end: 0x1003B,
    },
    Range {
        start: 0x1003C,
        end: 0x1003E,
    },
    Range {
        start: 0x1003F,
        end: 0x1004E,
    },
    Range {
        start: 0x10050,
        end: 0x1005E,
    },
    Range {
        start: 0x10080,
        end: 0x100FB,
    },
    Range {
        start: 0x101FD,
        end: 0x101FE,
    },
    Range {
        start: 0x10280,
        end: 0x1029D,
    },
    Range {
        start: 0x102A0,
        end: 0x102D1,
    },
    Range {
        start: 0x102E0,
        end: 0x102E1,
    },
    Range {
        start: 0x10300,
        end: 0x10320,
    },
    Range {
        start: 0x1032D,
        end: 0x10341,
    },
    Range {
        start: 0x10342,
        end: 0x1034A,
    },
    Range {
        start: 0x10350,
        end: 0x1037B,
    },
    Range {
        start: 0x10380,
        end: 0x1039E,
    },
    Range {
        start: 0x103A0,
        end: 0x103C4,
    },
    Range {
        start: 0x103C8,
        end: 0x103D0,
    },
    Range {
        start: 0x10428,
        end: 0x1049E,
    },
    Range {
        start: 0x104A0,
        end: 0x104AA,
    },
    Range {
        start: 0x104D8,
        end: 0x104FC,
    },
    Range {
        start: 0x10500,
        end: 0x10528,
    },
    Range {
        start: 0x10530,
        end: 0x10564,
    },
    Range {
        start: 0x10597,
        end: 0x105A2,
    },
    Range {
        start: 0x105A3,
        end: 0x105B2,
    },
    Range {
        start: 0x105B3,
        end: 0x105BA,
    },
    Range {
        start: 0x105BB,
        end: 0x105BD,
    },
    Range {
        start: 0x105C0,
        end: 0x105F4,
    },
    Range {
        start: 0x10600,
        end: 0x10737,
    },
    Range {
        start: 0x10740,
        end: 0x10756,
    },
    Range {
        start: 0x10760,
        end: 0x10768,
    },
    Range {
        start: 0x10780,
        end: 0x10781,
    },
    Range {
        start: 0x10800,
        end: 0x10806,
    },
    Range {
        start: 0x10808,
        end: 0x10809,
    },
    Range {
        start: 0x1080A,
        end: 0x10836,
    },
    Range {
        start: 0x10837,
        end: 0x10839,
    },
    Range {
        start: 0x1083C,
        end: 0x1083D,
    },
    Range {
        start: 0x1083F,
        end: 0x10856,
    },
    Range {
        start: 0x10860,
        end: 0x10877,
    },
    Range {
        start: 0x10880,
        end: 0x1089F,
    },
    Range {
        start: 0x108E0,
        end: 0x108F3,
    },
    Range {
        start: 0x108F4,
        end: 0x108F6,
    },
    Range {
        start: 0x10900,
        end: 0x10916,
    },
    Range {
        start: 0x10920,
        end: 0x1093A,
    },
    Range {
        start: 0x10940,
        end: 0x1095A,
    },
    Range {
        start: 0x10980,
        end: 0x109B8,
    },
    Range {
        start: 0x109BE,
        end: 0x109C0,
    },
    Range {
        start: 0x10A00,
        end: 0x10A04,
    },
    Range {
        start: 0x10A05,
        end: 0x10A07,
    },
    Range {
        start: 0x10A0C,
        end: 0x10A14,
    },
    Range {
        start: 0x10A15,
        end: 0x10A18,
    },
    Range {
        start: 0x10A19,
        end: 0x10A36,
    },
    Range {
        start: 0x10A38,
        end: 0x10A3B,
    },
    Range {
        start: 0x10A3F,
        end: 0x10A40,
    },
    Range {
        start: 0x10A60,
        end: 0x10A7D,
    },
    Range {
        start: 0x10A80,
        end: 0x10A9D,
    },
    Range {
        start: 0x10AC0,
        end: 0x10AC8,
    },
    Range {
        start: 0x10AC9,
        end: 0x10AE7,
    },
    Range {
        start: 0x10B00,
        end: 0x10B36,
    },
    Range {
        start: 0x10B40,
        end: 0x10B56,
    },
    Range {
        start: 0x10B60,
        end: 0x10B73,
    },
    Range {
        start: 0x10B80,
        end: 0x10B92,
    },
    Range {
        start: 0x10C00,
        end: 0x10C49,
    },
    Range {
        start: 0x10CC0,
        end: 0x10CF3,
    },
    Range {
        start: 0x10D00,
        end: 0x10D28,
    },
    Range {
        start: 0x10D30,
        end: 0x10D3A,
    },
    Range {
        start: 0x10D40,
        end: 0x10D50,
    },
    Range {
        start: 0x10D69,
        end: 0x10D6E,
    },
    Range {
        start: 0x10D6F,
        end: 0x10D86,
    },
    Range {
        start: 0x10E80,
        end: 0x10EAA,
    },
    Range {
        start: 0x10EAB,
        end: 0x10EAD,
    },
    Range {
        start: 0x10EB0,
        end: 0x10EB2,
    },
    Range {
        start: 0x10EC2,
        end: 0x10EC8,
    },
    Range {
        start: 0x10EFA,
        end: 0x10F1D,
    },
    Range {
        start: 0x10F27,
        end: 0x10F28,
    },
    Range {
        start: 0x10F30,
        end: 0x10F51,
    },
    Range {
        start: 0x10F70,
        end: 0x10F86,
    },
    Range {
        start: 0x10FB0,
        end: 0x10FC5,
    },
    Range {
        start: 0x10FE0,
        end: 0x10FF7,
    },
    Range {
        start: 0x11000,
        end: 0x11047,
    },
    Range {
        start: 0x11066,
        end: 0x11076,
    },
    Range {
        start: 0x1107F,
        end: 0x110BB,
    },
    Range {
        start: 0x110C2,
        end: 0x110C3,
    },
    Range {
        start: 0x110D0,
        end: 0x110E9,
    },
    Range {
        start: 0x110F0,
        end: 0x110FA,
    },
    Range {
        start: 0x11100,
        end: 0x11135,
    },
    Range {
        start: 0x11136,
        end: 0x11140,
    },
    Range {
        start: 0x11144,
        end: 0x11148,
    },
    Range {
        start: 0x11150,
        end: 0x11174,
    },
    Range {
        start: 0x11176,
        end: 0x11177,
    },
    Range {
        start: 0x11180,
        end: 0x111C5,
    },
    Range {
        start: 0x111C9,
        end: 0x111CD,
    },
    Range {
        start: 0x111CE,
        end: 0x111DB,
    },
    Range {
        start: 0x111DC,
        end: 0x111DD,
    },
    Range {
        start: 0x11200,
        end: 0x11212,
    },
    Range {
        start: 0x11213,
        end: 0x11238,
    },
    Range {
        start: 0x1123E,
        end: 0x11242,
    },
    Range {
        start: 0x11280,
        end: 0x11287,
    },
    Range {
        start: 0x11288,
        end: 0x11289,
    },
    Range {
        start: 0x1128A,
        end: 0x1128E,
    },
    Range {
        start: 0x1128F,
        end: 0x1129E,
    },
    Range {
        start: 0x1129F,
        end: 0x112A9,
    },
    Range {
        start: 0x112B0,
        end: 0x112EB,
    },
    Range {
        start: 0x112F0,
        end: 0x112FA,
    },
    Range {
        start: 0x11300,
        end: 0x11304,
    },
    Range {
        start: 0x11305,
        end: 0x1130D,
    },
    Range {
        start: 0x1130F,
        end: 0x11311,
    },
    Range {
        start: 0x11313,
        end: 0x11329,
    },
    Range {
        start: 0x1132A,
        end: 0x11331,
    },
    Range {
        start: 0x11332,
        end: 0x11334,
    },
    Range {
        start: 0x11335,
        end: 0x1133A,
    },
    Range {
        start: 0x1133B,
        end: 0x11345,
    },
    Range {
        start: 0x11347,
        end: 0x11349,
    },
    Range {
        start: 0x1134B,
        end: 0x1134E,
    },
    Range {
        start: 0x11350,
        end: 0x11351,
    },
    Range {
        start: 0x11357,
        end: 0x11358,
    },
    Range {
        start: 0x1135D,
        end: 0x11364,
    },
    Range {
        start: 0x11366,
        end: 0x1136D,
    },
    Range {
        start: 0x11370,
        end: 0x11375,
    },
    Range {
        start: 0x11380,
        end: 0x1138A,
    },
    Range {
        start: 0x1138B,
        end: 0x1138C,
    },
    Range {
        start: 0x1138E,
        end: 0x1138F,
    },
    Range {
        start: 0x11390,
        end: 0x113B6,
    },
    Range {
        start: 0x113B7,
        end: 0x113C1,
    },
    Range {
        start: 0x113C2,
        end: 0x113C3,
    },
    Range {
        start: 0x113C5,
        end: 0x113C6,
    },
    Range {
        start: 0x113C7,
        end: 0x113CB,
    },
    Range {
        start: 0x113CC,
        end: 0x113D4,
    },
    Range {
        start: 0x113E1,
        end: 0x113E3,
    },
    Range {
        start: 0x11400,
        end: 0x1144B,
    },
    Range {
        start: 0x11450,
        end: 0x1145A,
    },
    Range {
        start: 0x1145E,
        end: 0x11462,
    },
    Range {
        start: 0x11480,
        end: 0x114C6,
    },
    Range {
        start: 0x114C7,
        end: 0x114C8,
    },
    Range {
        start: 0x114D0,
        end: 0x114DA,
    },
    Range {
        start: 0x11580,
        end: 0x115B6,
    },
    Range {
        start: 0x115B8,
        end: 0x115C1,
    },
    Range {
        start: 0x115D8,
        end: 0x115DE,
    },
    Range {
        start: 0x11600,
        end: 0x11641,
    },
    Range {
        start: 0x11644,
        end: 0x11645,
    },
    Range {
        start: 0x11650,
        end: 0x1165A,
    },
    Range {
        start: 0x11680,
        end: 0x116B9,
    },
    Range {
        start: 0x116C0,
        end: 0x116CA,
    },
    Range {
        start: 0x116D0,
        end: 0x116E4,
    },
    Range {
        start: 0x11700,
        end: 0x1171B,
    },
    Range {
        start: 0x1171D,
        end: 0x1172C,
    },
    Range {
        start: 0x11730,
        end: 0x1173A,
    },
    Range {
        start: 0x11740,
        end: 0x11747,
    },
    Range {
        start: 0x11800,
        end: 0x1183B,
    },
    Range {
        start: 0x118C0,
        end: 0x118EA,
    },
    Range {
        start: 0x118FF,
        end: 0x11907,
    },
    Range {
        start: 0x11909,
        end: 0x1190A,
    },
    Range {
        start: 0x1190C,
        end: 0x11914,
    },
    Range {
        start: 0x11915,
        end: 0x11917,
    },
    Range {
        start: 0x11918,
        end: 0x11936,
    },
    Range {
        start: 0x11937,
        end: 0x11939,
    },
    Range {
        start: 0x1193B,
        end: 0x11944,
    },
    Range {
        start: 0x11950,
        end: 0x1195A,
    },
    Range {
        start: 0x119A0,
        end: 0x119A8,
    },
    Range {
        start: 0x119AA,
        end: 0x119D8,
    },
    Range {
        start: 0x119DA,
        end: 0x119E2,
    },
    Range {
        start: 0x119E3,
        end: 0x119E5,
    },
    Range {
        start: 0x11A00,
        end: 0x11A3F,
    },
    Range {
        start: 0x11A47,
        end: 0x11A48,
    },
    Range {
        start: 0x11A50,
        end: 0x11A9A,
    },
    Range {
        start: 0x11A9D,
        end: 0x11A9E,
    },
    Range {
        start: 0x11AB0,
        end: 0x11AF9,
    },
    Range {
        start: 0x11B60,
        end: 0x11B68,
    },
    Range {
        start: 0x11BC0,
        end: 0x11BE1,
    },
    Range {
        start: 0x11BF0,
        end: 0x11BFA,
    },
    Range {
        start: 0x11C00,
        end: 0x11C09,
    },
    Range {
        start: 0x11C0A,
        end: 0x11C37,
    },
    Range {
        start: 0x11C38,
        end: 0x11C41,
    },
    Range {
        start: 0x11C50,
        end: 0x11C5A,
    },
    Range {
        start: 0x11C72,
        end: 0x11C90,
    },
    Range {
        start: 0x11C92,
        end: 0x11CA8,
    },
    Range {
        start: 0x11CA9,
        end: 0x11CB7,
    },
    Range {
        start: 0x11D00,
        end: 0x11D07,
    },
    Range {
        start: 0x11D08,
        end: 0x11D0A,
    },
    Range {
        start: 0x11D0B,
        end: 0x11D37,
    },
    Range {
        start: 0x11D3A,
        end: 0x11D3B,
    },
    Range {
        start: 0x11D3C,
        end: 0x11D3E,
    },
    Range {
        start: 0x11D3F,
        end: 0x11D48,
    },
    Range {
        start: 0x11D50,
        end: 0x11D5A,
    },
    Range {
        start: 0x11D60,
        end: 0x11D66,
    },
    Range {
        start: 0x11D67,
        end: 0x11D69,
    },
    Range {
        start: 0x11D6A,
        end: 0x11D8F,
    },
    Range {
        start: 0x11D90,
        end: 0x11D92,
    },
    Range {
        start: 0x11D93,
        end: 0x11D99,
    },
    Range {
        start: 0x11DA0,
        end: 0x11DAA,
    },
    Range {
        start: 0x11DB0,
        end: 0x11DDC,
    },
    Range {
        start: 0x11DE0,
        end: 0x11DEA,
    },
    Range {
        start: 0x11EE0,
        end: 0x11EF7,
    },
    Range {
        start: 0x11F00,
        end: 0x11F11,
    },
    Range {
        start: 0x11F12,
        end: 0x11F3B,
    },
    Range {
        start: 0x11F3E,
        end: 0x11F43,
    },
    Range {
        start: 0x11F50,
        end: 0x11F5B,
    },
    Range {
        start: 0x11FB0,
        end: 0x11FB1,
    },
    Range {
        start: 0x12000,
        end: 0x1239A,
    },
    Range {
        start: 0x12480,
        end: 0x12544,
    },
    Range {
        start: 0x12F90,
        end: 0x12FF1,
    },
    Range {
        start: 0x13000,
        end: 0x13430,
    },
    Range {
        start: 0x13440,
        end: 0x13456,
    },
    Range {
        start: 0x13460,
        end: 0x143FB,
    },
    Range {
        start: 0x14400,
        end: 0x14647,
    },
    Range {
        start: 0x16100,
        end: 0x1613A,
    },
    Range {
        start: 0x16800,
        end: 0x16A39,
    },
    Range {
        start: 0x16A40,
        end: 0x16A5F,
    },
    Range {
        start: 0x16A60,
        end: 0x16A6A,
    },
    Range {
        start: 0x16A70,
        end: 0x16ABF,
    },
    Range {
        start: 0x16AC0,
        end: 0x16ACA,
    },
    Range {
        start: 0x16AD0,
        end: 0x16AEE,
    },
    Range {
        start: 0x16AF0,
        end: 0x16AF5,
    },
    Range {
        start: 0x16B00,
        end: 0x16B37,
    },
    Range {
        start: 0x16B40,
        end: 0x16B44,
    },
    Range {
        start: 0x16B50,
        end: 0x16B5A,
    },
    Range {
        start: 0x16B63,
        end: 0x16B78,
    },
    Range {
        start: 0x16B7D,
        end: 0x16B90,
    },
    Range {
        start: 0x16D40,
        end: 0x16D6D,
    },
    Range {
        start: 0x16D70,
        end: 0x16D7A,
    },
    Range {
        start: 0x16E60,
        end: 0x16E80,
    },
    Range {
        start: 0x16EBB,
        end: 0x16ED4,
    },
    Range {
        start: 0x16F00,
        end: 0x16F4B,
    },
    Range {
        start: 0x16F4F,
        end: 0x16F88,
    },
    Range {
        start: 0x16F8F,
        end: 0x16FA0,
    },
    Range {
        start: 0x16FE0,
        end: 0x16FE2,
    },
    Range {
        start: 0x16FE3,
        end: 0x16FE5,
    },
    Range {
        start: 0x16FF0,
        end: 0x16FF4,
    },
    Range {
        start: 0x17000,
        end: 0x18CD6,
    },
    Range {
        start: 0x18CFF,
        end: 0x18D1F,
    },
    Range {
        start: 0x18D80,
        end: 0x18DF3,
    },
    Range {
        start: 0x1AFF0,
        end: 0x1AFF4,
    },
    Range {
        start: 0x1AFF5,
        end: 0x1AFFC,
    },
    Range {
        start: 0x1AFFD,
        end: 0x1AFFF,
    },
    Range {
        start: 0x1B000,
        end: 0x1B123,
    },
    Range {
        start: 0x1B132,
        end: 0x1B133,
    },
    Range {
        start: 0x1B150,
        end: 0x1B153,
    },
    Range {
        start: 0x1B155,
        end: 0x1B156,
    },
    Range {
        start: 0x1B164,
        end: 0x1B168,
    },
    Range {
        start: 0x1B170,
        end: 0x1B2FC,
    },
    Range {
        start: 0x1BC00,
        end: 0x1BC6B,
    },
    Range {
        start: 0x1BC70,
        end: 0x1BC7D,
    },
    Range {
        start: 0x1BC80,
        end: 0x1BC89,
    },
    Range {
        start: 0x1BC90,
        end: 0x1BC9A,
    },
    Range {
        start: 0x1BC9D,
        end: 0x1BC9F,
    },
    Range {
        start: 0x1CF00,
        end: 0x1CF2E,
    },
    Range {
        start: 0x1CF30,
        end: 0x1CF47,
    },
    Range {
        start: 0x1DA00,
        end: 0x1DA37,
    },
    Range {
        start: 0x1DA3B,
        end: 0x1DA6D,
    },
    Range {
        start: 0x1DA75,
        end: 0x1DA76,
    },
    Range {
        start: 0x1DA84,
        end: 0x1DA85,
    },
    Range {
        start: 0x1DA9B,
        end: 0x1DAA0,
    },
    Range {
        start: 0x1DAA1,
        end: 0x1DAB0,
    },
    Range {
        start: 0x1DF00,
        end: 0x1DF1F,
    },
    Range {
        start: 0x1DF25,
        end: 0x1DF2B,
    },
    Range {
        start: 0x1E000,
        end: 0x1E007,
    },
    Range {
        start: 0x1E008,
        end: 0x1E019,
    },
    Range {
        start: 0x1E01B,
        end: 0x1E022,
    },
    Range {
        start: 0x1E023,
        end: 0x1E025,
    },
    Range {
        start: 0x1E026,
        end: 0x1E02B,
    },
    Range {
        start: 0x1E08F,
        end: 0x1E090,
    },
    Range {
        start: 0x1E100,
        end: 0x1E12D,
    },
    Range {
        start: 0x1E130,
        end: 0x1E13E,
    },
    Range {
        start: 0x1E140,
        end: 0x1E14A,
    },
    Range {
        start: 0x1E14E,
        end: 0x1E14F,
    },
    Range {
        start: 0x1E290,
        end: 0x1E2AF,
    },
    Range {
        start: 0x1E2C0,
        end: 0x1E2FA,
    },
    Range {
        start: 0x1E4D0,
        end: 0x1E4FA,
    },
    Range {
        start: 0x1E5D0,
        end: 0x1E5FB,
    },
    Range {
        start: 0x1E6C0,
        end: 0x1E6DF,
    },
    Range {
        start: 0x1E6E0,
        end: 0x1E6F6,
    },
    Range {
        start: 0x1E6FE,
        end: 0x1E700,
    },
    Range {
        start: 0x1E7E0,
        end: 0x1E7E7,
    },
    Range {
        start: 0x1E7E8,
        end: 0x1E7EC,
    },
    Range {
        start: 0x1E7ED,
        end: 0x1E7EF,
    },
    Range {
        start: 0x1E7F0,
        end: 0x1E7FF,
    },
    Range {
        start: 0x1E800,
        end: 0x1E8C5,
    },
    Range {
        start: 0x1E8D0,
        end: 0x1E8D7,
    },
    Range {
        start: 0x1E922,
        end: 0x1E94C,
    },
    Range {
        start: 0x1E950,
        end: 0x1E95A,
    },
    Range {
        start: 0x20000,
        end: 0x2A6E0,
    },
    Range {
        start: 0x2A700,
        end: 0x2B81E,
    },
    Range {
        start: 0x2B820,
        end: 0x2CEAE,
    },
    Range {
        start: 0x2CEB0,
        end: 0x2EBE1,
    },
    Range {
        start: 0x2EBF0,
        end: 0x2EE5E,
    },
    Range {
        start: 0x30000,
        end: 0x3134B,
    },
    Range {
        start: 0x31350,
        end: 0x3347A,
    },
];

pub const CLASS_CONTEXTJ: &[Range] = &[Range {
    start: 0x200C,
    end: 0x200E,
}];

pub const CLASS_CONTEXTO: &[Range] = &[
    Range {
        start: 0xB7,
        end: 0xB8,
    },
    Range {
        start: 0x375,
        end: 0x376,
    },
    Range {
        start: 0x5F3,
        end: 0x5F5,
    },
    Range {
        start: 0x660,
        end: 0x66A,
    },
    Range {
        start: 0x6F0,
        end: 0x6FA,
    },
    Range {
        start: 0x30FB,
        end: 0x30FC,
    },
];

pub const SCRIPT_GREEK: &[Range] = &[
    Range {
        start: 0x370,
        end: 0x374,
    },
    Range {
        start: 0x375,
        end: 0x378,
    },
    Range {
        start: 0x37A,
        end: 0x37E,
    },
    Range {
        start: 0x37F,
        end: 0x380,
    },
    Range {
        start: 0x384,
        end: 0x385,
    },
    Range {
        start: 0x386,
        end: 0x387,
    },
    Range {
        start: 0x388,
        end: 0x38B,
    },
    Range {
        start: 0x38C,
        end: 0x38D,
    },
    Range {
        start: 0x38E,
        end: 0x3A2,
    },
    Range {
        start: 0x3A3,
        end: 0x3E2,
    },
    Range {
        start: 0x3F0,
        end: 0x400,
    },
    Range {
        start: 0x1D26,
        end: 0x1D2B,
    },
    Range {
        start: 0x1D5D,
        end: 0x1D62,
    },
    Range {
        start: 0x1D66,
        end: 0x1D6B,
    },
    Range {
        start: 0x1DBF,
        end: 0x1DC0,
    },
    Range {
        start: 0x1F00,
        end: 0x1F16,
    },
    Range {
        start: 0x1F18,
        end: 0x1F1E,
    },
    Range {
        start: 0x1F20,
        end: 0x1F46,
    },
    Range {
        start: 0x1F48,
        end: 0x1F4E,
    },
    Range {
        start: 0x1F50,
        end: 0x1F58,
    },
    Range {
        start: 0x1F59,
        end: 0x1F5A,
    },
    Range {
        start: 0x1F5B,
        end: 0x1F5C,
    },
    Range {
        start: 0x1F5D,
        end: 0x1F5E,
    },
    Range {
        start: 0x1F5F,
        end: 0x1F7E,
    },
    Range {
        start: 0x1F80,
        end: 0x1FB5,
    },
    Range {
        start: 0x1FB6,
        end: 0x1FC5,
    },
    Range {
        start: 0x1FC6,
        end: 0x1FD4,
    },
    Range {
        start: 0x1FD6,
        end: 0x1FDC,
    },
    Range {
        start: 0x1FDD,
        end: 0x1FF0,
    },
    Range {
        start: 0x1FF2,
        end: 0x1FF5,
    },
    Range {
        start: 0x1FF6,
        end: 0x1FFF,
    },
    Range {
        start: 0x2126,
        end: 0x2127,
    },
    Range {
        start: 0xAB65,
        end: 0xAB66,
    },
    Range {
        start: 0x10140,
        end: 0x1018F,
    },
    Range {
        start: 0x101A0,
        end: 0x101A1,
    },
    Range {
        start: 0x1D200,
        end: 0x1D246,
    },
];

pub const SCRIPT_HAN: &[Range] = &[
    Range {
        start: 0x2E80,
        end: 0x2E9A,
    },
    Range {
        start: 0x2E9B,
        end: 0x2EF4,
    },
    Range {
        start: 0x2F00,
        end: 0x2FD6,
    },
    Range {
        start: 0x3005,
        end: 0x3006,
    },
    Range {
        start: 0x3007,
        end: 0x3008,
    },
    Range {
        start: 0x3021,
        end: 0x302A,
    },
    Range {
        start: 0x3038,
        end: 0x303C,
    },
    Range {
        start: 0x3400,
        end: 0x4DC0,
    },
    Range {
        start: 0x4E00,
        end: 0xA000,
    },
    Range {
        start: 0xF900,
        end: 0xFA6E,
    },
    Range {
        start: 0xFA70,
        end: 0xFADA,
    },
    Range {
        start: 0x16FE2,
        end: 0x16FE4,
    },
    Range {
        start: 0x16FF0,
        end: 0x16FF7,
    },
    Range {
        start: 0x20000,
        end: 0x2A6E0,
    },
    Range {
        start: 0x2A700,
        end: 0x2B81E,
    },
    Range {
        start: 0x2B820,
        end: 0x2CEAE,
    },
    Range {
        start: 0x2CEB0,
        end: 0x2EBE1,
    },
    Range {
        start: 0x2EBF0,
        end: 0x2EE5E,
    },
    Range {
        start: 0x2F800,
        end: 0x2FA1E,
    },
    Range {
        start: 0x30000,
        end: 0x3134B,
    },
    Range {
        start: 0x31350,
        end: 0x3347A,
    },
];

pub const SCRIPT_HEBREW: &[Range] = &[
    Range {
        start: 0x591,
        end: 0x5C8,
    },
    Range {
        start: 0x5D0,
        end: 0x5EB,
    },
    Range {
        start: 0x5EF,
        end: 0x5F5,
    },
    Range {
        start: 0xFB1D,
        end: 0xFB37,
    },
    Range {
        start: 0xFB38,
        end: 0xFB3D,
    },
    Range {
        start: 0xFB3E,
        end: 0xFB3F,
    },
    Range {
        start: 0xFB40,
        end: 0xFB42,
    },
    Range {
        start: 0xFB43,
        end: 0xFB45,
    },
    Range {
        start: 0xFB46,
        end: 0xFB50,
    },
];

pub const SCRIPT_HIRAGANA: &[Range] = &[
    Range {
        start: 0x3041,
        end: 0x3097,
    },
    Range {
        start: 0x309D,
        end: 0x30A0,
    },
    Range {
        start: 0x1B001,
        end: 0x1B120,
    },
    Range {
        start: 0x1B132,
        end: 0x1B133,
    },
    Range {
        start: 0x1B150,
        end: 0x1B153,
    },
    Range {
        start: 0x1F200,
        end: 0x1F201,
    },
];

pub const SCRIPT_KATAKANA: &[Range] = &[
    Range {
        start: 0x30A1,
        end: 0x30FB,
    },
    Range {
        start: 0x30FD,
        end: 0x3100,
    },
    Range {
        start: 0x31F0,
        end: 0x3200,
    },
    Range {
        start: 0x32D0,
        end: 0x32FF,
    },
    Range {
        start: 0x3300,
        end: 0x3358,
    },
    Range {
        start: 0xFF66,
        end: 0xFF70,
    },
    Range {
        start: 0xFF71,
        end: 0xFF9E,
    },
    Range {
        start: 0x1AFF0,
        end: 0x1AFF4,
    },
    Range {
        start: 0x1AFF5,
        end: 0x1AFFC,
    },
    Range {
        start: 0x1AFFD,
        end: 0x1AFFF,
    },
    Range {
        start: 0x1B000,
        end: 0x1B001,
    },
    Range {
        start: 0x1B120,
        end: 0x1B123,
    },
    Range {
        start: 0x1B155,
        end: 0x1B156,
    },
    Range {
        start: 0x1B164,
        end: 0x1B168,
    },
];

pub const JOINING_C: &[Range] = &[
    Range {
        start: 0x640,
        end: 0x641,
    },
    Range {
        start: 0x7FA,
        end: 0x7FB,
    },
    Range {
        start: 0x883,
        end: 0x886,
    },
    Range {
        start: 0x180A,
        end: 0x180B,
    },
    Range {
        start: 0x200D,
        end: 0x200E,
    },
];

pub const JOINING_D: &[Range] = &[
    Range {
        start: 0x620,
        end: 0x621,
    },
    Range {
        start: 0x626,
        end: 0x627,
    },
    Range {
        start: 0x628,
        end: 0x629,
    },
    Range {
        start: 0x62A,
        end: 0x62F,
    },
    Range {
        start: 0x633,
        end: 0x640,
    },
    Range {
        start: 0x641,
        end: 0x648,
    },
    Range {
        start: 0x649,
        end: 0x64B,
    },
    Range {
        start: 0x66E,
        end: 0x670,
    },
    Range {
        start: 0x678,
        end: 0x688,
    },
    Range {
        start: 0x69A,
        end: 0x6C0,
    },
    Range {
        start: 0x6C1,
        end: 0x6C3,
    },
    Range {
        start: 0x6CC,
        end: 0x6CD,
    },
    Range {
        start: 0x6CE,
        end: 0x6CF,
    },
    Range {
        start: 0x6D0,
        end: 0x6D2,
    },
    Range {
        start: 0x6FA,
        end: 0x6FD,
    },
    Range {
        start: 0x6FF,
        end: 0x700,
    },
    Range {
        start: 0x712,
        end: 0x715,
    },
    Range {
        start: 0x71A,
        end: 0x71E,
    },
    Range {
        start: 0x71F,
        end: 0x728,
    },
    Range {
        start: 0x729,
        end: 0x72A,
    },
    Range {
        start: 0x72B,
        end: 0x72C,
    },
    Range {
        start: 0x72D,
        end: 0x72F,
    },
    Range {
        start: 0x74E,
        end: 0x759,
    },
    Range {
        start: 0x75C,
        end: 0x76B,
    },
    Range {
        start: 0x76D,
        end: 0x771,
    },
    Range {
        start: 0x772,
        end: 0x773,
    },
    Range {
        start: 0x775,
        end: 0x778,
    },
    Range {
        start: 0x77A,
        end: 0x780,
    },
    Range {
        start: 0x7CA,
        end: 0x7EB,
    },
    Range {
        start: 0x841,
        end: 0x846,
    },
    Range {
        start: 0x848,
        end: 0x849,
    },
    Range {
        start: 0x84A,
        end: 0x854,
    },
    Range {
        start: 0x855,
        end: 0x856,
    },
    Range {
        start: 0x860,
        end: 0x861,
    },
    Range {
        start: 0x862,
        end: 0x866,
    },
    Range {
        start: 0x868,
        end: 0x869,
    },
    Range {
        start: 0x886,
        end: 0x887,
    },
    Range {
        start: 0x889,
        end: 0x88E,
    },
    Range {
        start: 0x88F,
        end: 0x890,
    },
    Range {
        start: 0x8A0,
        end: 0x8AA,
    },
    Range {
        start: 0x8AF,
        end: 0x8B1,
    },
    Range {
        start: 0x8B3,
        end: 0x8B9,
    },
    Range {
        start: 0x8BA,
        end: 0x8C9,
    },
    Range {
        start: 0x1807,
        end: 0x1808,
    },
    Range {
        start: 0x1820,
        end: 0x1879,
    },
    Range {
        start: 0x1887,
        end: 0x18A9,
    },
    Range {
        start: 0x18AA,
        end: 0x18AB,
    },
    Range {
        start: 0xA840,
        end: 0xA872,
    },
    Range {
        start: 0x10AC0,
        end: 0x10AC5,
    },
    Range {
        start: 0x10AD3,
        end: 0x10AD7,
    },
    Range {
        start: 0x10AD8,
        end: 0x10ADD,
    },
    Range {
        start: 0x10ADE,
        end: 0x10AE1,
    },
    Range {
        start: 0x10AEB,
        end: 0x10AEF,
    },
    Range {
        start: 0x10B80,
        end: 0x10B81,
    },
    Range {
        start: 0x10B82,
        end: 0x10B83,
    },
    Range {
        start: 0x10B86,
        end: 0x10B89,
    },
    Range {
        start: 0x10B8A,
        end: 0x10B8C,
    },
    Range {
        start: 0x10B8D,
        end: 0x10B8E,
    },
    Range {
        start: 0x10B90,
        end: 0x10B91,
    },
    Range {
        start: 0x10BAD,
        end: 0x10BAF,
    },
    Range {
        start: 0x10D01,
        end: 0x10D22,
    },
    Range {
        start: 0x10D23,
        end: 0x10D24,
    },
    Range {
        start: 0x10EC3,
        end: 0x10EC5,
    },
    Range {
        start: 0x10EC6,
        end: 0x10EC8,
    },
    Range {
        start: 0x10F30,
        end: 0x10F33,
    },
    Range {
        start: 0x10F34,
        end: 0x10F45,
    },
    Range {
        start: 0x10F51,
        end: 0x10F54,
    },
    Range {
        start: 0x10F70,
        end: 0x10F74,
    },
    Range {
        start: 0x10F76,
        end: 0x10F82,
    },
    Range {
        start: 0x10FB0,
        end: 0x10FB1,
    },
    Range {
        start: 0x10FB2,
        end: 0x10FB4,
    },
    Range {
        start: 0x10FB8,
        end: 0x10FB9,
    },
    Range {
        start: 0x10FBB,
        end: 0x10FBD,
    },
    Range {
        start: 0x10FBE,
        end: 0x10FC0,
    },
    Range {
        start: 0x10FC1,
        end: 0x10FC2,
    },
    Range {
        start: 0x10FC4,
        end: 0x10FC5,
    },
    Range {
        start: 0x10FCA,
        end: 0x10FCB,
    },
    Range {
        start: 0x1E900,
        end: 0x1E944,
    },
];

pub const JOINING_L: &[Range] = &[
    Range {
        start: 0xA872,
        end: 0xA873,
    },
    Range {
        start: 0x10ACD,
        end: 0x10ACE,
    },
    Range {
        start: 0x10AD7,
        end: 0x10AD8,
    },
    Range {
        start: 0x10D00,
        end: 0x10D01,
    },
    Range {
        start: 0x10FCB,
        end: 0x10FCC,
    },
];

pub const JOINING_R: &[Range] = &[
    Range {
        start: 0x622,
        end: 0x626,
    },
    Range {
        start: 0x627,
        end: 0x628,
    },
    Range {
        start: 0x629,
        end: 0x62A,
    },
    Range {
        start: 0x62F,
        end: 0x633,
    },
    Range {
        start: 0x648,
        end: 0x649,
    },
    Range {
        start: 0x671,
        end: 0x674,
    },
    Range {
        start: 0x675,
        end: 0x678,
    },
    Range {
        start: 0x688,
        end: 0x69A,
    },
    Range {
        start: 0x6C0,
        end: 0x6C1,
    },
    Range {
        start: 0x6C3,
        end: 0x6CC,
    },
    Range {
        start: 0x6CD,
        end: 0x6CE,
    },
    Range {
        start: 0x6CF,
        end: 0x6D0,
    },
    Range {
        start: 0x6D2,
        end: 0x6D4,
    },
    Range {
        start: 0x6D5,
        end: 0x6D6,
    },
    Range {
        start: 0x6EE,
        end: 0x6F0,
    },
    Range {
        start: 0x710,
        end: 0x711,
    },
    Range {
        start: 0x715,
        end: 0x71A,
    },
    Range {
        start: 0x71E,
        end: 0x71F,
    },
    Range {
        start: 0x728,
        end: 0x729,
    },
    Range {
        start: 0x72A,
        end: 0x72B,
    },
    Range {
        start: 0x72C,
        end: 0x72D,
    },
    Range {
        start: 0x72F,
        end: 0x730,
    },
    Range {
        start: 0x74D,
        end: 0x74E,
    },
    Range {
        start: 0x759,
        end: 0x75C,
    },
    Range {
        start: 0x76B,
        end: 0x76D,
    },
    Range {
        start: 0x771,
        end: 0x772,
    },
    Range {
        start: 0x773,
        end: 0x775,
    },
    Range {
        start: 0x778,
        end: 0x77A,
    },
    Range {
        start: 0x840,
        end: 0x841,
    },
    Range {
        start: 0x846,
        end: 0x848,
    },
    Range {
        start: 0x849,
        end: 0x84A,
    },
    Range {
        start: 0x854,
        end: 0x855,
    },
    Range {
        start: 0x856,
        end: 0x859,
    },
    Range {
        start: 0x867,
        end: 0x868,
    },
    Range {
        start: 0x869,
        end: 0x86B,
    },
    Range {
        start: 0x870,
        end: 0x883,
    },
    Range {
        start: 0x88E,
        end: 0x88F,
    },
    Range {
        start: 0x8AA,
        end: 0x8AD,
    },
    Range {
        start: 0x8AE,
        end: 0x8AF,
    },
    Range {
        start: 0x8B1,
        end: 0x8B3,
    },
    Range {
        start: 0x8B9,
        end: 0x8BA,
    },
    Range {
        start: 0x10AC5,
        end: 0x10AC6,
    },
    Range {
        start: 0x10AC7,
        end: 0x10AC8,
    },
    Range {
        start: 0x10AC9,
        end: 0x10ACB,
    },
    Range {
        start: 0x10ACE,
        end: 0x10AD3,
    },
    Range {
        start: 0x10ADD,
        end: 0x10ADE,
    },
    Range {
        start: 0x10AE1,
        end: 0x10AE2,
    },
    Range {
        start: 0x10AE4,
        end: 0x10AE5,
    },
    Range {
        start: 0x10AEF,
        end: 0x10AF0,
    },
    Range {
        start: 0x10B81,
        end: 0x10B82,
    },
    Range {
        start: 0x10B83,
        end: 0x10B86,
    },
    Range {
        start: 0x10B89,
        end: 0x10B8A,
    },
    Range {
        start: 0x10B8C,
        end: 0x10B8D,
    },
    Range {
        start: 0x10B8E,
        end: 0x10B90,
    },
    Range {
        start: 0x10B91,
        end: 0x10B92,
    },
    Range {
        start: 0x10BA9,
        end: 0x10BAD,
    },
    Range {
        start: 0x10D22,
        end: 0x10D23,
    },
    Range {
        start: 0x10EC2,
        end: 0x10EC3,
    },
    Range {
        start: 0x10F33,
        end: 0x10F34,
    },
    Range {
        start: 0x10F54,
        end: 0x10F55,
    },
    Range {
        start: 0x10F74,
        end: 0x10F76,
    },
    Range {
        start: 0x10FB4,
        end: 0x10FB7,
    },
    Range {
        start: 0x10FB9,
        end: 0x10FBB,
    },
    Range {
        start: 0x10FBD,
        end: 0x10FBE,
    },
    Range {
        start: 0x10FC2,
        end: 0x10FC4,
    },
    Range {
        start: 0x10FC9,
        end: 0x10FCA,
    },
];

pub const JOINING_T: &[Range] = &[
    Range {
        start: 0xAD,
        end: 0xAE,
    },
    Range {
        start: 0x300,
        end: 0x370,
    },
    Range {
        start: 0x483,
        end: 0x48A,
    },
    Range {
        start: 0x591,
        end: 0x5BE,
    },
    Range {
        start: 0x5BF,
        end: 0x5C0,
    },
    Range {
        start: 0x5C1,
        end: 0x5C3,
    },
    Range {
        start: 0x5C4,
        end: 0x5C6,
    },
    Range {
        start: 0x5C7,
        end: 0x5C8,
    },
    Range {
        start: 0x610,
        end: 0x61B,
    },
    Range {
        start: 0x61C,
        end: 0x61D,
    },
    Range {
        start: 0x64B,
        end: 0x660,
    },
    Range {
        start: 0x670,
        end: 0x671,
    },
    Range {
        start: 0x6D6,
        end: 0x6DD,
    },
    Range {
        start: 0x6DF,
        end: 0x6E5,
    },
    Range {
        start: 0x6E7,
        end: 0x6E9,
    },
    Range {
        start: 0x6EA,
        end: 0x6EE,
    },
    Range {
        start: 0x70F,
        end: 0x710,
    },
    Range {
        start: 0x711,
        end: 0x712,
    },
    Range {
        start: 0x730,
        end: 0x74B,
    },
    Range {
        start: 0x7A6,
        end: 0x7B1,
    },
    Range {
        start: 0x7EB,
        end: 0x7F4,
    },
    Range {
        start: 0x7FD,
        end: 0x7FE,
    },
    Range {
        start: 0x816,
        end: 0x81A,
    },
    Range {
        start: 0x81B,
        end: 0x824,
    },
    Range {
        start: 0x825,
        end: 0x828,
    },
    Range {
        start: 0x829,
        end: 0x82E,
    },
    Range {
        start: 0x859,
        end: 0x85C,
    },
    Range {
        start: 0x897,
        end: 0x8A0,
    },
    Range {
        start: 0x8CA,
        end: 0x8E2,
    },
    Range {
        start: 0x8E3,
        end: 0x903,
    },
    Range {
        start: 0x93A,
        end: 0x93B,
    },
    Range {
        start: 0x93C,
        end: 0x93D,
    },
    Range {
        start: 0x941,
        end: 0x949,
    },
    Range {
        start: 0x94D,
        end: 0x94E,
    },
    Range {
        start: 0x951,
        end: 0x958,
    },
    Range {
        start: 0x962,
        end: 0x964,
    },
    Range {
        start: 0x981,
        end: 0x982,
    },
    Range {
        start: 0x9BC,
        end: 0x9BD,
    },
    Range {
        start: 0x9C1,
        end: 0x9C5,
    },
    Range {
        start: 0x9CD,
        end: 0x9CE,
    },
    Range {
        start: 0x9E2,
        end: 0x9E4,
    },
    Range {
        start: 0x9FE,
        end: 0x9FF,
    },
    Range {
        start: 0xA01,
        end: 0xA03,
    },
    Range {
        start: 0xA3C,
        end: 0xA3D,
    },
    Range {
        start: 0xA41,
        end: 0xA43,
    },
    Range {
        start: 0xA47,
        end: 0xA49,
    },
    Range {
        start: 0xA4B,
        end: 0xA4E,
    },
    Range {
        start: 0xA51,
        end: 0xA52,
    },
    Range {
        start: 0xA70,
        end: 0xA72,
    },
    Range {
        start: 0xA75,
        end: 0xA76,
    },
    Range {
        start: 0xA81,
        end: 0xA83,
    },
    Range {
        start: 0xABC,
        end: 0xABD,
    },
    Range {
        start: 0xAC1,
        end: 0xAC6,
    },
    Range {
        start: 0xAC7,
        end: 0xAC9,
    },
    Range {
        start: 0xACD,
        end: 0xACE,
    },
    Range {
        start: 0xAE2,
        end: 0xAE4,
    },
    Range {
        start: 0xAFA,
        end: 0xB00,
    },
    Range {
        start: 0xB01,
        end: 0xB02,
    },
    Range {
        start: 0xB3C,
        end: 0xB3D,
    },
    Range {
        start: 0xB3F,
        end: 0xB40,
    },
    Range {
        start: 0xB41,
        end: 0xB45,
    },
    Range {
        start: 0xB4D,
        end: 0xB4E,
    },
    Range {
        start: 0xB55,
        end: 0xB57,
    },
    Range {
        start: 0xB62,
        end: 0xB64,
    },
    Range {
        start: 0xB82,
        end: 0xB83,
    },
    Range {
        start: 0xBC0,
        end: 0xBC1,
    },
    Range {
        start: 0xBCD,
        end: 0xBCE,
    },
    Range {
        start: 0xC00,
        end: 0xC01,
    },
    Range {
        start: 0xC04,
        end: 0xC05,
    },
    Range {
        start: 0xC3C,
        end: 0xC3D,
    },
    Range {
        start: 0xC3E,
        end: 0xC41,
    },
    Range {
        start: 0xC46,
        end: 0xC49,
    },
    Range {
        start: 0xC4A,
        end: 0xC4E,
    },
    Range {
        start: 0xC55,
        end: 0xC57,
    },
    Range {
        start: 0xC62,
        end: 0xC64,
    },
    Range {
        start: 0xC81,
        end: 0xC82,
    },
    Range {
        start: 0xCBC,
        end: 0xCBD,
    },
    Range {
        start: 0xCBF,
        end: 0xCC0,
    },
    Range {
        start: 0xCC6,
        end: 0xCC7,
    },
    Range {
        start: 0xCCC,
        end: 0xCCE,
    },
    Range {
        start: 0xCE2,
        end: 0xCE4,
    },
    Range {
        start: 0xD00,
        end: 0xD02,
    },
    Range {
        start: 0xD3B,
        end: 0xD3D,
    },
    Range {
        start: 0xD41,
        end: 0xD45,
    },
    Range {
        start: 0xD4D,
        end: 0xD4E,
    },
    Range {
        start: 0xD62,
        end: 0xD64,
    },
    Range {
        start: 0xD81,
        end: 0xD82,
    },
    Range {
        start: 0xDCA,
        end: 0xDCB,
    },
    Range {
        start: 0xDD2,
        end: 0xDD5,
    },
    Range {
        start: 0xDD6,
        end: 0xDD7,
    },
    Range {
        start: 0xE31,
        end: 0xE32,
    },
    Range {
        start: 0xE34,
        end: 0xE3B,
    },
    Range {
        start: 0xE47,
        end: 0xE4F,
    },
    Range {
        start: 0xEB1,
        end: 0xEB2,
    },
    Range {
        start: 0xEB4,
        end: 0xEBD,
    },
    Range {
        start: 0xEC8,
        end: 0xECF,
    },
    Range {
        start: 0xF18,
        end: 0xF1A,
    },
    Range {
        start: 0xF35,
        end: 0xF36,
    },
    Range {
        start: 0xF37,
        end: 0xF38,
    },
    Range {
        start: 0xF39,
        end: 0xF3A,
    },
    Range {
        start: 0xF71,
        end: 0xF7F,
    },
    Range {
        start: 0xF80,
        end: 0xF85,
    },
    Range {
        start: 0xF86,
        end: 0xF88,
    },
    Range {
        start: 0xF8D,
        end: 0xF98,
    },
    Range {
        start: 0xF99,
        end: 0xFBD,
    },
    Range {
        start: 0xFC6,
        end: 0xFC7,
    },
    Range {
        start: 0x102D,
        end: 0x1031,
    },
    Range {
        start: 0x1032,
        end: 0x1038,
    },
    Range {
        start: 0x1039,
        end: 0x103B,
    },
    Range {
        start: 0x103D,
        end: 0x103F,
    },
    Range {
        start: 0x1058,
        end: 0x105A,
    },
    Range {
        start: 0x105E,
        end: 0x1061,
    },
    Range {
        start: 0x1071,
        end: 0x1075,
    },
    Range {
        start: 0x1082,
        end: 0x1083,
    },
    Range {
        start: 0x1085,
        end: 0x1087,
    },
    Range {
        start: 0x108D,
        end: 0x108E,
    },
    Range {
        start: 0x109D,
        end: 0x109E,
    },
    Range {
        start: 0x135D,
        end: 0x1360,
    },
    Range {
        start: 0x1712,
        end: 0x1715,
    },
    Range {
        start: 0x1732,
        end: 0x1734,
    },
    Range {
        start: 0x1752,
        end: 0x1754,
    },
    Range {
        start: 0x1772,
        end: 0x1774,
    },
    Range {
        start: 0x17B4,
        end: 0x17B6,
    },
    Range {
        start: 0x17B7,
        end: 0x17BE,
    },
    Range {
        start: 0x17C6,
        end: 0x17C7,
    },
    Range {
        start: 0x17C9,
        end: 0x17D4,
    },
    Range {
        start: 0x17DD,
        end: 0x17DE,
    },
    Range {
        start: 0x180B,
        end: 0x180E,
    },
    Range {
        start: 0x180F,
        end: 0x1810,
    },
    Range {
        start: 0x1885,
        end: 0x1887,
    },
    Range {
        start: 0x18A9,
        end: 0x18AA,
    },
    Range {
        start: 0x1920,
        end: 0x1923,
    },
    Range {
        start: 0x1927,
        end: 0x1929,
    },
    Range {
        start: 0x1932,
        end: 0x1933,
    },
    Range {
        start: 0x1939,
        end: 0x193C,
    },
    Range {
        start: 0x1A17,
        end: 0x1A19,
    },
    Range {
        start: 0x1A1B,
        end: 0x1A1C,
    },
    Range {
        start: 0x1A56,
        end: 0x1A57,
    },
    Range {
        start: 0x1A58,
        end: 0x1A5F,
    },
    Range {
        start: 0x1A60,
        end: 0x1A61,
    },
    Range {
        start: 0x1A62,
        end: 0x1A63,
    },
    Range {
        start: 0x1A65,
        end: 0x1A6D,
    },
    Range {
        start: 0x1A73,
        end: 0x1A7D,
    },
    Range {
        start: 0x1A7F,
        end: 0x1A80,
    },
    Range {
        start: 0x1AB0,
        end: 0x1ADE,
    },
    Range {
        start: 0x1AE0,
        end: 0x1AEC,
    },
    Range {
        start: 0x1B00,
        end: 0x1B04,
    },
    Range {
        start: 0x1B34,
        end: 0x1B35,
    },
    Range {
        start: 0x1B36,
        end: 0x1B3B,
    },
    Range {
        start: 0x1B3C,
        end: 0x1B3D,
    },
    Range {
        start: 0x1B42,
        end: 0x1B43,
    },
    Range {
        start: 0x1B6B,
        end: 0x1B74,
    },
    Range {
        start: 0x1B80,
        end: 0x1B82,
    },
    Range {
        start: 0x1BA2,
        end: 0x1BA6,
    },
    Range {
        start: 0x1BA8,
        end: 0x1BAA,
    },
    Range {
        start: 0x1BAB,
        end: 0x1BAE,
    },
    Range {
        start: 0x1BE6,
        end: 0x1BE7,
    },
    Range {
        start: 0x1BE8,
        end: 0x1BEA,
    },
    Range {
        start: 0x1BED,
        end: 0x1BEE,
    },
    Range {
        start: 0x1BEF,
        end: 0x1BF2,
    },
    Range {
        start: 0x1C2C,
        end: 0x1C34,
    },
    Range {
        start: 0x1C36,
        end: 0x1C38,
    },
    Range {
        start: 0x1CD0,
        end: 0x1CD3,
    },
    Range {
        start: 0x1CD4,
        end: 0x1CE1,
    },
    Range {
        start: 0x1CE2,
        end: 0x1CE9,
    },
    Range {
        start: 0x1CED,
        end: 0x1CEE,
    },
    Range {
        start: 0x1CF4,
        end: 0x1CF5,
    },
    Range {
        start: 0x1CF8,
        end: 0x1CFA,
    },
    Range {
        start: 0x1DC0,
        end: 0x1E00,
    },
    Range {
        start: 0x200B,
        end: 0x200C,
    },
    Range {
        start: 0x200E,
        end: 0x2010,
    },
    Range {
        start: 0x202A,
        end: 0x202F,
    },
    Range {
        start: 0x2060,
        end: 0x2065,
    },
    Range {
        start: 0x206A,
        end: 0x2070,
    },
    Range {
        start: 0x20D0,
        end: 0x20F1,
    },
    Range {
        start: 0x2CEF,
        end: 0x2CF2,
    },
    Range {
        start: 0x2D7F,
        end: 0x2D80,
    },
    Range {
        start: 0x2DE0,
        end: 0x2E00,
    },
    Range {
        start: 0x302A,
        end: 0x302E,
    },
    Range {
        start: 0x3099,
        end: 0x309B,
    },
    Range {
        start: 0xA66F,
        end: 0xA673,
    },
    Range {
        start: 0xA674,
        end: 0xA67E,
    },
    Range {
        start: 0xA69E,
        end: 0xA6A0,
    },
    Range {
        start: 0xA6F0,
        end: 0xA6F2,
    },
    Range {
        start: 0xA802,
        end: 0xA803,
    },
    Range {
        start: 0xA806,
        end: 0xA807,
    },
    Range {
        start: 0xA80B,
        end: 0xA80C,
    },
    Range {
        start: 0xA825,
        end: 0xA827,
    },
    Range {
        start: 0xA82C,
        end: 0xA82D,
    },
    Range {
        start: 0xA8C4,
        end: 0xA8C6,
    },
    Range {
        start: 0xA8E0,
        end: 0xA8F2,
    },
    Range {
        start: 0xA8FF,
        end: 0xA900,
    },
    Range {
        start: 0xA926,
        end: 0xA92E,
    },
    Range {
        start: 0xA947,
        end: 0xA952,
    },
    Range {
        start: 0xA980,
        end: 0xA983,
    },
    Range {
        start: 0xA9B3,
        end: 0xA9B4,
    },
    Range {
        start: 0xA9B6,
        end: 0xA9BA,
    },
    Range {
        start: 0xA9BC,
        end: 0xA9BE,
    },
    Range {
        start: 0xA9E5,
        end: 0xA9E6,
    },
    Range {
        start: 0xAA29,
        end: 0xAA2F,
    },
    Range {
        start: 0xAA31,
        end: 0xAA33,
    },
    Range {
        start: 0xAA35,
        end: 0xAA37,
    },
    Range {
        start: 0xAA43,
        end: 0xAA44,
    },
    Range {
        start: 0xAA4C,
        end: 0xAA4D,
    },
    Range {
        start: 0xAA7C,
        end: 0xAA7D,
    },
    Range {
        start: 0xAAB0,
        end: 0xAAB1,
    },
    Range {
        start: 0xAAB2,
        end: 0xAAB5,
    },
    Range {
        start: 0xAAB7,
        end: 0xAAB9,
    },
    Range {
        start: 0xAABE,
        end: 0xAAC0,
    },
    Range {
        start: 0xAAC1,
        end: 0xAAC2,
    },
    Range {
        start: 0xAAEC,
        end: 0xAAEE,
    },
    Range {
        start: 0xAAF6,
        end: 0xAAF7,
    },
    Range {
        start: 0xABE5,
        end: 0xABE6,
    },
    Range {
        start: 0xABE8,
        end: 0xABE9,
    },
    Range {
        start: 0xABED,
        end: 0xABEE,
    },
    Range {
        start: 0xFB1E,
        end: 0xFB1F,
    },
    Range {
        start: 0xFE00,
        end: 0xFE10,
    },
    Range {
        start: 0xFE20,
        end: 0xFE30,
    },
    Range {
        start: 0xFEFF,
        end: 0xFF00,
    },
    Range {
        start: 0xFFF9,
        end: 0xFFFC,
    },
    Range {
        start: 0x101FD,
        end: 0x101FE,
    },
    Range {
        start: 0x102E0,
        end: 0x102E1,
    },
    Range {
        start: 0x10376,
        end: 0x1037B,
    },
    Range {
        start: 0x10A01,
        end: 0x10A04,
    },
    Range {
        start: 0x10A05,
        end: 0x10A07,
    },
    Range {
        start: 0x10A0C,
        end: 0x10A10,
    },
    Range {
        start: 0x10A38,
        end: 0x10A3B,
    },
    Range {
        start: 0x10A3F,
        end: 0x10A40,
    },
    Range {
        start: 0x10AE5,
        end: 0x10AE7,
    },
    Range {
        start: 0x10D24,
        end: 0x10D28,
    },
    Range {
        start: 0x10D69,
        end: 0x10D6E,
    },
    Range {
        start: 0x10EAB,
        end: 0x10EAD,
    },
    Range {
        start: 0x10EFA,
        end: 0x10F00,
    },
    Range {
        start: 0x10F46,
        end: 0x10F51,
    },
    Range {
        start: 0x10F82,
        end: 0x10F86,
    },
    Range {
        start: 0x11001,
        end: 0x11002,
    },
    Range {
        start: 0x11038,
        end: 0x11047,
    },
    Range {
        start: 0x11070,
        end: 0x11071,
    },
    Range {
        start: 0x11073,
        end: 0x11075,
    },
    Range {
        start: 0x1107F,
        end: 0x11082,
    },
    Range {
        start: 0x110B3,
        end: 0x110B7,
    },
    Range {
        start: 0x110B9,
        end: 0x110BB,
    },
    Range {
        start: 0x110C2,
        end: 0x110C3,
    },
    Range {
        start: 0x11100,
        end: 0x11103,
    },
    Range {
        start: 0x11127,
        end: 0x1112C,
    },
    Range {
        start: 0x1112D,
        end: 0x11135,
    },
    Range {
        start: 0x11173,
        end: 0x11174,
    },
    Range {
        start: 0x11180,
        end: 0x11182,
    },
    Range {
        start: 0x111B6,
        end: 0x111BF,
    },
    Range {
        start: 0x111C9,
        end: 0x111CD,
    },
    Range {
        start: 0x111CF,
        end: 0x111D0,
    },
    Range {
        start: 0x1122F,
        end: 0x11232,
    },
    Range {
        start: 0x11234,
        end: 0x11235,
    },
    Range {
        start: 0x11236,
        end: 0x11238,
    },
    Range {
        start: 0x1123E,
        end: 0x1123F,
    },
    Range {
        start: 0x11241,
        end: 0x11242,
    },
    Range {
        start: 0x112DF,
        end: 0x112E0,
    },
    Range {
        start: 0x112E3,
        end: 0x112EB,
    },
    Range {
        start: 0x11300,
        end: 0x11302,
    },
    Range {
        start: 0x1133B,
        end: 0x1133D,
    },
    Range {
        start: 0x11340,
        end: 0x11341,
    },
    Range {
        start: 0x11366,
        end: 0x1136D,
    },
    Range {
        start: 0x11370,
        end: 0x11375,
    },
    Range {
        start: 0x113BB,
        end: 0x113C1,
    },
    Range {
        start: 0x113CE,
        end: 0x113CF,
    },
    Range {
        start: 0x113D0,
        end: 0x113D1,
    },
    Range {
        start: 0x113D2,
        end: 0x113D3,
    },
    Range {
        start: 0x113E1,
        end: 0x113E3,
    },
    Range {
        start: 0x11438,
        end: 0x11440,
    },
    Range {
        start: 0x11442,
        end: 0x11445,
    },
    Range {
        start: 0x11446,
        end: 0x11447,
    },
    Range {
        start: 0x1145E,
        end: 0x1145F,
    },
    Range {
        start: 0x114B3,
        end: 0x114B9,
    },
    Range {
        start: 0x114BA,
        end: 0x114BB,
    },
    Range {
        start: 0x114BF,
        end: 0x114C1,
    },
    Range {
        start: 0x114C2,
        end: 0x114C4,
    },
    Range {
        start: 0x115B2,
        end: 0x115B6,
    },
    Range {
        start: 0x115BC,
        end: 0x115BE,
    },
    Range {
        start: 0x115BF,
        end: 0x115C1,
    },
    Range {
        start: 0x115DC,
        end: 0x115DE,
    },
    Range {
        start: 0x11633,
        end: 0x1163B,
    },
    Range {
        start: 0x1163D,
        end: 0x1163E,
    },
    Range {
        start: 0x1163F,
        end: 0x11641,
    },
    Range {
        start: 0x116AB,
        end: 0x116AC,
    },
    Range {
        start: 0x116AD,
        end: 0x116AE,
    },
    Range {
        start: 0x116B0,
        end: 0x116B6,
    },
    Range {
        start: 0x116B7,
        end: 0x116B8,
    },
    Range {
        start: 0x1171D,
        end: 0x1171E,
    },
    Range {
        start: 0x1171F,
        end: 0x11720,
    },
    Range {
        start: 0x11722,
        end: 0x11726,
    },
    Range {
        start: 0x11727,
        end: 0x1172C,
    },
    Range {
        start: 0x1182F,
        end: 0x11838,
    },
    Range {
        start: 0x11839,
        end: 0x1183B,
    },
    Range {
        start: 0x1193B,
        end: 0x1193D,
    },
    Range {
        start: 0x1193E,
        end: 0x1193F,
    },
    Range {
        start: 0x11943,
        end: 0x11944,
    },
    Range {
        start: 0x119D4,
        end: 0x119D8,
    },
    Range {
        start: 0x119DA,
        end: 0x119DC,
    },
    Range {
        start: 0x119E0,
        end: 0x119E1,
    },
    Range {
        start: 0x11A01,
        end: 0x11A0B,
    },
    Range {
        start: 0x11A33,
        end: 0x11A39,
    },
    Range {
        start: 0x11A3B,
        end: 0x11A3F,
    },
    Range {
        start: 0x11A47,
        end: 0x11A48,
    },
    Range {
        start: 0x11A51,
        end: 0x11A57,
    },
    Range {
        start: 0x11A59,
        end: 0x11A5C,
    },
    Range {
        start: 0x11A8A,
        end: 0x11A97,
    },
    Range {
        start: 0x11A98,
        end: 0x11A9A,
    },
    Range {
        start: 0x11B60,
        end: 0x11B61,
    },
    Range {
        start: 0x11B62,
        end: 0x11B65,
    },
    Range {
        start: 0x11B66,
        end: 0x11B67,
    },
    Range {
        start: 0x11C30,
        end: 0x11C37,
    },
    Range {
        start: 0x11C38,
        end: 0x11C3E,
    },
    Range {
        start: 0x11C3F,
        end: 0x11C40,
    },
    Range {
        start: 0x11C92,
        end: 0x11CA8,
    },
    Range {
        start: 0x11CAA,
        end: 0x11CB1,
    },
    Range {
        start: 0x11CB2,
        end: 0x11CB4,
    },
    Range {
        start: 0x11CB5,
        end: 0x11CB7,
    },
    Range {
        start: 0x11D31,
        end: 0x11D37,
    },
    Range {
        start: 0x11D3A,
        end: 0x11D3B,
    },
    Range {
        start: 0x11D3C,
        end: 0x11D3E,
    },
    Range {
        start: 0x11D3F,
        end: 0x11D46,
    },
    Range {
        start: 0x11D47,
        end: 0x11D48,
    },
    Range {
        start: 0x11D90,
        end: 0x11D92,
    },
    Range {
        start: 0x11D95,
        end: 0x11D96,
    },
    Range {
        start: 0x11D97,
        end: 0x11D98,
    },
    Range {
        start: 0x11EF3,
        end: 0x11EF5,
    },
    Range {
        start: 0x11F00,
        end: 0x11F02,
    },
    Range {
        start: 0x11F36,
        end: 0x11F3B,
    },
    Range {
        start: 0x11F40,
        end: 0x11F41,
    },
    Range {
        start: 0x11F42,
        end: 0x11F43,
    },
    Range {
        start: 0x11F5A,
        end: 0x11F5B,
    },
    Range {
        start: 0x13430,
        end: 0x13441,
    },
    Range {
        start: 0x13447,
        end: 0x13456,
    },
    Range {
        start: 0x1611E,
        end: 0x1612A,
    },
    Range {
        start: 0x1612D,
        end: 0x16130,
    },
    Range {
        start: 0x16AF0,
        end: 0x16AF5,
    },
    Range {
        start: 0x16B30,
        end: 0x16B37,
    },
    Range {
        start: 0x16F4F,
        end: 0x16F50,
    },
    Range {
        start: 0x16F8F,
        end: 0x16F93,
    },
    Range {
        start: 0x16FE4,
        end: 0x16FE5,
    },
    Range {
        start: 0x1BC9D,
        end: 0x1BC9F,
    },
    Range {
        start: 0x1BCA0,
        end: 0x1BCA4,
    },
    Range {
        start: 0x1CF00,
        end: 0x1CF2E,
    },
    Range {
        start: 0x1CF30,
        end: 0x1CF47,
    },
    Range {
        start: 0x1D167,
        end: 0x1D16A,
    },
    Range {
        start: 0x1D173,
        end: 0x1D183,
    },
    Range {
        start: 0x1D185,
        end: 0x1D18C,
    },
    Range {
        start: 0x1D1AA,
        end: 0x1D1AE,
    },
    Range {
        start: 0x1D242,
        end: 0x1D245,
    },
    Range {
        start: 0x1DA00,
        end: 0x1DA37,
    },
    Range {
        start: 0x1DA3B,
        end: 0x1DA6D,
    },
    Range {
        start: 0x1DA75,
        end: 0x1DA76,
    },
    Range {
        start: 0x1DA84,
        end: 0x1DA85,
    },
    Range {
        start: 0x1DA9B,
        end: 0x1DAA0,
    },
    Range {
        start: 0x1DAA1,
        end: 0x1DAB0,
    },
    Range {
        start: 0x1E000,
        end: 0x1E007,
    },
    Range {
        start: 0x1E008,
        end: 0x1E019,
    },
    Range {
        start: 0x1E01B,
        end: 0x1E022,
    },
    Range {
        start: 0x1E023,
        end: 0x1E025,
    },
    Range {
        start: 0x1E026,
        end: 0x1E02B,
    },
    Range {
        start: 0x1E08F,
        end: 0x1E090,
    },
    Range {
        start: 0x1E130,
        end: 0x1E137,
    },
    Range {
        start: 0x1E2AE,
        end: 0x1E2AF,
    },
    Range {
        start: 0x1E2EC,
        end: 0x1E2F0,
    },
    Range {
        start: 0x1E4EC,
        end: 0x1E4F0,
    },
    Range {
        start: 0x1E5EE,
        end: 0x1E5F0,
    },
    Range {
        start: 0x1E6E3,
        end: 0x1E6E4,
    },
    Range {
        start: 0x1E6E6,
        end: 0x1E6E7,
    },
    Range {
        start: 0x1E6EE,
        end: 0x1E6F0,
    },
    Range {
        start: 0x1E6F5,
        end: 0x1E6F6,
    },
    Range {
        start: 0x1E8D0,
        end: 0x1E8D7,
    },
    Range {
        start: 0x1E944,
        end: 0x1E94C,
    },
    Range {
        start: 0xE0001,
        end: 0xE0002,
    },
    Range {
        start: 0xE0020,
        end: 0xE0080,
    },
    Range {
        start: 0xE0100,
        end: 0xE01F0,
    },
];

pub const BIDI: &[PropertyRange<&str>] = &[
    PropertyRange {
        start: 0x20,
        end: 0x21,
        value: "WS",
    },
    PropertyRange {
        start: 0x21,
        end: 0x23,
        value: "ON",
    },
    PropertyRange {
        start: 0x23,
        end: 0x26,
        value: "ET",
    },
    PropertyRange {
        start: 0x26,
        end: 0x2B,
        value: "ON",
    },
    PropertyRange {
        start: 0x2B,
        end: 0x2C,
        value: "ES",
    },
    PropertyRange {
        start: 0x2C,
        end: 0x2D,
        value: "CS",
    },
    PropertyRange {
        start: 0x2D,
        end: 0x2E,
        value: "ES",
    },
    PropertyRange {
        start: 0x2E,
        end: 0x30,
        value: "CS",
    },
    PropertyRange {
        start: 0x30,
        end: 0x3A,
        value: "EN",
    },
    PropertyRange {
        start: 0x3A,
        end: 0x3B,
        value: "CS",
    },
    PropertyRange {
        start: 0x3B,
        end: 0x41,
        value: "ON",
    },
    PropertyRange {
        start: 0x41,
        end: 0x5B,
        value: "L",
    },
    PropertyRange {
        start: 0x5B,
        end: 0x61,
        value: "ON",
    },
    PropertyRange {
        start: 0x61,
        end: 0x7B,
        value: "L",
    },
    PropertyRange {
        start: 0x7B,
        end: 0x7F,
        value: "ON",
    },
    PropertyRange {
        start: 0xA0,
        end: 0xA1,
        value: "CS",
    },
    PropertyRange {
        start: 0xA1,
        end: 0xA2,
        value: "ON",
    },
    PropertyRange {
        start: 0xA2,
        end: 0xA6,
        value: "ET",
    },
    PropertyRange {
        start: 0xA6,
        end: 0xAA,
        value: "ON",
    },
    PropertyRange {
        start: 0xAA,
        end: 0xAB,
        value: "L",
    },
    PropertyRange {
        start: 0xAB,
        end: 0xAD,
        value: "ON",
    },
    PropertyRange {
        start: 0xAD,
        end: 0xAE,
        value: "BN",
    },
    PropertyRange {
        start: 0xAE,
        end: 0xB0,
        value: "ON",
    },
    PropertyRange {
        start: 0xB0,
        end: 0xB2,
        value: "ET",
    },
    PropertyRange {
        start: 0xB2,
        end: 0xB4,
        value: "EN",
    },
    PropertyRange {
        start: 0xB4,
        end: 0xB5,
        value: "ON",
    },
    PropertyRange {
        start: 0xB5,
        end: 0xB6,
        value: "L",
    },
    PropertyRange {
        start: 0xB6,
        end: 0xB9,
        value: "ON",
    },
    PropertyRange {
        start: 0xB9,
        end: 0xBA,
        value: "EN",
    },
    PropertyRange {
        start: 0xBA,
        end: 0xBB,
        value: "L",
    },
    PropertyRange {
        start: 0xBB,
        end: 0xC0,
        value: "ON",
    },
    PropertyRange {
        start: 0xC0,
        end: 0xD7,
        value: "L",
    },
    PropertyRange {
        start: 0xD7,
        end: 0xD8,
        value: "ON",
    },
    PropertyRange {
        start: 0xD8,
        end: 0xF7,
        value: "L",
    },
    PropertyRange {
        start: 0xF7,
        end: 0xF8,
        value: "ON",
    },
    PropertyRange {
        start: 0xF8,
        end: 0x2B9,
        value: "L",
    },
    PropertyRange {
        start: 0x2B9,
        end: 0x2BB,
        value: "ON",
    },
    PropertyRange {
        start: 0x2BB,
        end: 0x2C2,
        value: "L",
    },
    PropertyRange {
        start: 0x2C2,
        end: 0x2D0,
        value: "ON",
    },
    PropertyRange {
        start: 0x2D0,
        end: 0x2D2,
        value: "L",
    },
    PropertyRange {
        start: 0x2D2,
        end: 0x2E0,
        value: "ON",
    },
    PropertyRange {
        start: 0x2E0,
        end: 0x2E5,
        value: "L",
    },
    PropertyRange {
        start: 0x2E5,
        end: 0x2EE,
        value: "ON",
    },
    PropertyRange {
        start: 0x2EE,
        end: 0x2EF,
        value: "L",
    },
    PropertyRange {
        start: 0x2EF,
        end: 0x300,
        value: "ON",
    },
    PropertyRange {
        start: 0x300,
        end: 0x370,
        value: "NSM",
    },
    PropertyRange {
        start: 0x370,
        end: 0x374,
        value: "L",
    },
    PropertyRange {
        start: 0x374,
        end: 0x376,
        value: "ON",
    },
    PropertyRange {
        start: 0x376,
        end: 0x378,
        value: "L",
    },
    PropertyRange {
        start: 0x37A,
        end: 0x37E,
        value: "L",
    },
    PropertyRange {
        start: 0x37E,
        end: 0x37F,
        value: "ON",
    },
    PropertyRange {
        start: 0x37F,
        end: 0x380,
        value: "L",
    },
    PropertyRange {
        start: 0x384,
        end: 0x386,
        value: "ON",
    },
    PropertyRange {
        start: 0x386,
        end: 0x387,
        value: "L",
    },
    PropertyRange {
        start: 0x387,
        end: 0x388,
        value: "ON",
    },
    PropertyRange {
        start: 0x388,
        end: 0x38B,
        value: "L",
    },
    PropertyRange {
        start: 0x38C,
        end: 0x38D,
        value: "L",
    },
    PropertyRange {
        start: 0x38E,
        end: 0x3A2,
        value: "L",
    },
    PropertyRange {
        start: 0x3A3,
        end: 0x3F6,
        value: "L",
    },
    PropertyRange {
        start: 0x3F6,
        end: 0x3F7,
        value: "ON",
    },
    PropertyRange {
        start: 0x3F7,
        end: 0x483,
        value: "L",
    },
    PropertyRange {
        start: 0x483,
        end: 0x48A,
        value: "NSM",
    },
    PropertyRange {
        start: 0x48A,
        end: 0x530,
        value: "L",
    },
    PropertyRange {
        start: 0x531,
        end: 0x557,
        value: "L",
    },
    PropertyRange {
        start: 0x559,
        end: 0x58A,
        value: "L",
    },
    PropertyRange {
        start: 0x58A,
        end: 0x58B,
        value: "ON",
    },
    PropertyRange {
        start: 0x58D,
        end: 0x58F,
        value: "ON",
    },
    PropertyRange {
        start: 0x58F,
        end: 0x590,
        value: "ET",
    },
    PropertyRange {
        start: 0x591,
        end: 0x5BE,
        value: "NSM",
    },
    PropertyRange {
        start: 0x5BE,
        end: 0x5BF,
        value: "R",
    },
    PropertyRange {
        start: 0x5BF,
        end: 0x5C0,
        value: "NSM",
    },
    PropertyRange {
        start: 0x5C0,
        end: 0x5C1,
        value: "R",
    },
    PropertyRange {
        start: 0x5C1,
        end: 0x5C3,
        value: "NSM",
    },
    PropertyRange {
        start: 0x5C3,
        end: 0x5C4,
        value: "R",
    },
    PropertyRange {
        start: 0x5C4,
        end: 0x5C6,
        value: "NSM",
    },
    PropertyRange {
        start: 0x5C6,
        end: 0x5C7,
        value: "R",
    },
    PropertyRange {
        start: 0x5C7,
        end: 0x5C8,
        value: "NSM",
    },
    PropertyRange {
        start: 0x5D0,
        end: 0x5EB,
        value: "R",
    },
    PropertyRange {
        start: 0x5EF,
        end: 0x5F5,
        value: "R",
    },
    PropertyRange {
        start: 0x600,
        end: 0x606,
        value: "AN",
    },
    PropertyRange {
        start: 0x606,
        end: 0x608,
        value: "ON",
    },
    PropertyRange {
        start: 0x608,
        end: 0x609,
        value: "AL",
    },
    PropertyRange {
        start: 0x609,
        end: 0x60B,
        value: "ET",
    },
    PropertyRange {
        start: 0x60B,
        end: 0x60C,
        value: "AL",
    },
    PropertyRange {
        start: 0x60C,
        end: 0x60D,
        value: "CS",
    },
    PropertyRange {
        start: 0x60D,
        end: 0x60E,
        value: "AL",
    },
    PropertyRange {
        start: 0x60E,
        end: 0x610,
        value: "ON",
    },
    PropertyRange {
        start: 0x610,
        end: 0x61B,
        value: "NSM",
    },
    PropertyRange {
        start: 0x61B,
        end: 0x64B,
        value: "AL",
    },
    PropertyRange {
        start: 0x64B,
        end: 0x660,
        value: "NSM",
    },
    PropertyRange {
        start: 0x660,
        end: 0x66A,
        value: "AN",
    },
    PropertyRange {
        start: 0x66A,
        end: 0x66B,
        value: "ET",
    },
    PropertyRange {
        start: 0x66B,
        end: 0x66D,
        value: "AN",
    },
    PropertyRange {
        start: 0x66D,
        end: 0x670,
        value: "AL",
    },
    PropertyRange {
        start: 0x670,
        end: 0x671,
        value: "NSM",
    },
    PropertyRange {
        start: 0x671,
        end: 0x6D6,
        value: "AL",
    },
    PropertyRange {
        start: 0x6D6,
        end: 0x6DD,
        value: "NSM",
    },
    PropertyRange {
        start: 0x6DD,
        end: 0x6DE,
        value: "AN",
    },
    PropertyRange {
        start: 0x6DE,
        end: 0x6DF,
        value: "ON",
    },
    PropertyRange {
        start: 0x6DF,
        end: 0x6E5,
        value: "NSM",
    },
    PropertyRange {
        start: 0x6E5,
        end: 0x6E7,
        value: "AL",
    },
    PropertyRange {
        start: 0x6E7,
        end: 0x6E9,
        value: "NSM",
    },
    PropertyRange {
        start: 0x6E9,
        end: 0x6EA,
        value: "ON",
    },
    PropertyRange {
        start: 0x6EA,
        end: 0x6EE,
        value: "NSM",
    },
    PropertyRange {
        start: 0x6EE,
        end: 0x6F0,
        value: "AL",
    },
    PropertyRange {
        start: 0x6F0,
        end: 0x6FA,
        value: "EN",
    },
    PropertyRange {
        start: 0x6FA,
        end: 0x70E,
        value: "AL",
    },
    PropertyRange {
        start: 0x70F,
        end: 0x711,
        value: "AL",
    },
    PropertyRange {
        start: 0x711,
        end: 0x712,
        value: "NSM",
    },
    PropertyRange {
        start: 0x712,
        end: 0x730,
        value: "AL",
    },
    PropertyRange {
        start: 0x730,
        end: 0x74B,
        value: "NSM",
    },
    PropertyRange {
        start: 0x74D,
        end: 0x7A6,
        value: "AL",
    },
    PropertyRange {
        start: 0x7A6,
        end: 0x7B1,
        value: "NSM",
    },
    PropertyRange {
        start: 0x7B1,
        end: 0x7B2,
        value: "AL",
    },
    PropertyRange {
        start: 0x7C0,
        end: 0x7EB,
        value: "R",
    },
    PropertyRange {
        start: 0x7EB,
        end: 0x7F4,
        value: "NSM",
    },
    PropertyRange {
        start: 0x7F4,
        end: 0x7F6,
        value: "R",
    },
    PropertyRange {
        start: 0x7F6,
        end: 0x7FA,
        value: "ON",
    },
    PropertyRange {
        start: 0x7FA,
        end: 0x7FB,
        value: "R",
    },
    PropertyRange {
        start: 0x7FD,
        end: 0x7FE,
        value: "NSM",
    },
    PropertyRange {
        start: 0x7FE,
        end: 0x816,
        value: "R",
    },
    PropertyRange {
        start: 0x816,
        end: 0x81A,
        value: "NSM",
    },
    PropertyRange {
        start: 0x81A,
        end: 0x81B,
        value: "R",
    },
    PropertyRange {
        start: 0x81B,
        end: 0x824,
        value: "NSM",
    },
    PropertyRange {
        start: 0x824,
        end: 0x825,
        value: "R",
    },
    PropertyRange {
        start: 0x825,
        end: 0x828,
        value: "NSM",
    },
    PropertyRange {
        start: 0x828,
        end: 0x829,
        value: "R",
    },
    PropertyRange {
        start: 0x829,
        end: 0x82E,
        value: "NSM",
    },
    PropertyRange {
        start: 0x830,
        end: 0x83F,
        value: "R",
    },
    PropertyRange {
        start: 0x840,
        end: 0x859,
        value: "R",
    },
    PropertyRange {
        start: 0x859,
        end: 0x85C,
        value: "NSM",
    },
    PropertyRange {
        start: 0x85E,
        end: 0x85F,
        value: "R",
    },
    PropertyRange {
        start: 0x860,
        end: 0x86B,
        value: "AL",
    },
    PropertyRange {
        start: 0x870,
        end: 0x88F,
        value: "AL",
    },
    PropertyRange {
        start: 0x890,
        end: 0x892,
        value: "AN",
    },
    PropertyRange {
        start: 0x897,
        end: 0x8A0,
        value: "NSM",
    },
    PropertyRange {
        start: 0x8A0,
        end: 0x8CA,
        value: "AL",
    },
    PropertyRange {
        start: 0x8CA,
        end: 0x8E2,
        value: "NSM",
    },
    PropertyRange {
        start: 0x8E2,
        end: 0x8E3,
        value: "AN",
    },
    PropertyRange {
        start: 0x8E3,
        end: 0x903,
        value: "NSM",
    },
    PropertyRange {
        start: 0x903,
        end: 0x93A,
        value: "L",
    },
    PropertyRange {
        start: 0x93A,
        end: 0x93B,
        value: "NSM",
    },
    PropertyRange {
        start: 0x93B,
        end: 0x93C,
        value: "L",
    },
    PropertyRange {
        start: 0x93C,
        end: 0x93D,
        value: "NSM",
    },
    PropertyRange {
        start: 0x93D,
        end: 0x941,
        value: "L",
    },
    PropertyRange {
        start: 0x941,
        end: 0x949,
        value: "NSM",
    },
    PropertyRange {
        start: 0x949,
        end: 0x94D,
        value: "L",
    },
    PropertyRange {
        start: 0x94D,
        end: 0x94E,
        value: "NSM",
    },
    PropertyRange {
        start: 0x94E,
        end: 0x951,
        value: "L",
    },
    PropertyRange {
        start: 0x951,
        end: 0x958,
        value: "NSM",
    },
    PropertyRange {
        start: 0x958,
        end: 0x962,
        value: "L",
    },
    PropertyRange {
        start: 0x962,
        end: 0x964,
        value: "NSM",
    },
    PropertyRange {
        start: 0x964,
        end: 0x981,
        value: "L",
    },
    PropertyRange {
        start: 0x981,
        end: 0x982,
        value: "NSM",
    },
    PropertyRange {
        start: 0x982,
        end: 0x984,
        value: "L",
    },
    PropertyRange {
        start: 0x985,
        end: 0x98D,
        value: "L",
    },
    PropertyRange {
        start: 0x98F,
        end: 0x991,
        value: "L",
    },
    PropertyRange {
        start: 0x993,
        end: 0x9A9,
        value: "L",
    },
    PropertyRange {
        start: 0x9AA,
        end: 0x9B1,
        value: "L",
    },
    PropertyRange {
        start: 0x9B2,
        end: 0x9B3,
        value: "L",
    },
    PropertyRange {
        start: 0x9B6,
        end: 0x9BA,
        value: "L",
    },
    PropertyRange {
        start: 0x9BC,
        end: 0x9BD,
        value: "NSM",
    },
    PropertyRange {
        start: 0x9BD,
        end: 0x9C1,
        value: "L",
    },
    PropertyRange {
        start: 0x9C1,
        end: 0x9C5,
        value: "NSM",
    },
    PropertyRange {
        start: 0x9C7,
        end: 0x9C9,
        value: "L",
    },
    PropertyRange {
        start: 0x9CB,
        end: 0x9CD,
        value: "L",
    },
    PropertyRange {
        start: 0x9CD,
        end: 0x9CE,
        value: "NSM",
    },
    PropertyRange {
        start: 0x9CE,
        end: 0x9CF,
        value: "L",
    },
    PropertyRange {
        start: 0x9D7,
        end: 0x9D8,
        value: "L",
    },
    PropertyRange {
        start: 0x9DC,
        end: 0x9DE,
        value: "L",
    },
    PropertyRange {
        start: 0x9DF,
        end: 0x9E2,
        value: "L",
    },
    PropertyRange {
        start: 0x9E2,
        end: 0x9E4,
        value: "NSM",
    },
    PropertyRange {
        start: 0x9E6,
        end: 0x9F2,
        value: "L",
    },
    PropertyRange {
        start: 0x9F2,
        end: 0x9F4,
        value: "ET",
    },
    PropertyRange {
        start: 0x9F4,
        end: 0x9FB,
        value: "L",
    },
    PropertyRange {
        start: 0x9FB,
        end: 0x9FC,
        value: "ET",
    },
    PropertyRange {
        start: 0x9FC,
        end: 0x9FE,
        value: "L",
    },
    PropertyRange {
        start: 0x9FE,
        end: 0x9FF,
        value: "NSM",
    },
    PropertyRange {
        start: 0xA01,
        end: 0xA03,
        value: "NSM",
    },
    PropertyRange {
        start: 0xA03,
        end: 0xA04,
        value: "L",
    },
    PropertyRange {
        start: 0xA05,
        end: 0xA0B,
        value: "L",
    },
    PropertyRange {
        start: 0xA0F,
        end: 0xA11,
        value: "L",
    },
    PropertyRange {
        start: 0xA13,
        end: 0xA29,
        value: "L",
    },
    PropertyRange {
        start: 0xA2A,
        end: 0xA31,
        value: "L",
    },
    PropertyRange {
        start: 0xA32,
        end: 0xA34,
        value: "L",
    },
    PropertyRange {
        start: 0xA35,
        end: 0xA37,
        value: "L",
    },
    PropertyRange {
        start: 0xA38,
        end: 0xA3A,
        value: "L",
    },
    PropertyRange {
        start: 0xA3C,
        end: 0xA3D,
        value: "NSM",
    },
    PropertyRange {
        start: 0xA3E,
        end: 0xA41,
        value: "L",
    },
    PropertyRange {
        start: 0xA41,
        end: 0xA43,
        value: "NSM",
    },
    PropertyRange {
        start: 0xA47,
        end: 0xA49,
        value: "NSM",
    },
    PropertyRange {
        start: 0xA4B,
        end: 0xA4E,
        value: "NSM",
    },
    PropertyRange {
        start: 0xA51,
        end: 0xA52,
        value: "NSM",
    },
    PropertyRange {
        start: 0xA59,
        end: 0xA5D,
        value: "L",
    },
    PropertyRange {
        start: 0xA5E,
        end: 0xA5F,
        value: "L",
    },
    PropertyRange {
        start: 0xA66,
        end: 0xA70,
        value: "L",
    },
    PropertyRange {
        start: 0xA70,
        end: 0xA72,
        value: "NSM",
    },
    PropertyRange {
        start: 0xA72,
        end: 0xA75,
        value: "L",
    },
    PropertyRange {
        start: 0xA75,
        end: 0xA76,
        value: "NSM",
    },
    PropertyRange {
        start: 0xA76,
        end: 0xA77,
        value: "L",
    },
    PropertyRange {
        start: 0xA81,
        end: 0xA83,
        value: "NSM",
    },
    PropertyRange {
        start: 0xA83,
        end: 0xA84,
        value: "L",
    },
    PropertyRange {
        start: 0xA85,
        end: 0xA8E,
        value: "L",
    },
    PropertyRange {
        start: 0xA8F,
        end: 0xA92,
        value: "L",
    },
    PropertyRange {
        start: 0xA93,
        end: 0xAA9,
        value: "L",
    },
    PropertyRange {
        start: 0xAAA,
        end: 0xAB1,
        value: "L",
    },
    PropertyRange {
        start: 0xAB2,
        end: 0xAB4,
        value: "L",
    },
    PropertyRange {
        start: 0xAB5,
        end: 0xABA,
        value: "L",
    },
    PropertyRange {
        start: 0xABC,
        end: 0xABD,
        value: "NSM",
    },
    PropertyRange {
        start: 0xABD,
        end: 0xAC1,
        value: "L",
    },
    PropertyRange {
        start: 0xAC1,
        end: 0xAC6,
        value: "NSM",
    },
    PropertyRange {
        start: 0xAC7,
        end: 0xAC9,
        value: "NSM",
    },
    PropertyRange {
        start: 0xAC9,
        end: 0xACA,
        value: "L",
    },
    PropertyRange {
        start: 0xACB,
        end: 0xACD,
        value: "L",
    },
    PropertyRange {
        start: 0xACD,
        end: 0xACE,
        value: "NSM",
    },
    PropertyRange {
        start: 0xAD0,
        end: 0xAD1,
        value: "L",
    },
    PropertyRange {
        start: 0xAE0,
        end: 0xAE2,
        value: "L",
    },
    PropertyRange {
        start: 0xAE2,
        end: 0xAE4,
        value: "NSM",
    },
    PropertyRange {
        start: 0xAE6,
        end: 0xAF1,
        value: "L",
    },
    PropertyRange {
        start: 0xAF1,
        end: 0xAF2,
        value: "ET",
    },
    PropertyRange {
        start: 0xAF9,
        end: 0xAFA,
        value: "L",
    },
    PropertyRange {
        start: 0xAFA,
        end: 0xB00,
        value: "NSM",
    },
    PropertyRange {
        start: 0xB01,
        end: 0xB02,
        value: "NSM",
    },
    PropertyRange {
        start: 0xB02,
        end: 0xB04,
        value: "L",
    },
    PropertyRange {
        start: 0xB05,
        end: 0xB0D,
        value: "L",
    },
    PropertyRange {
        start: 0xB0F,
        end: 0xB11,
        value: "L",
    },
    PropertyRange {
        start: 0xB13,
        end: 0xB29,
        value: "L",
    },
    PropertyRange {
        start: 0xB2A,
        end: 0xB31,
        value: "L",
    },
    PropertyRange {
        start: 0xB32,
        end: 0xB34,
        value: "L",
    },
    PropertyRange {
        start: 0xB35,
        end: 0xB3A,
        value: "L",
    },
    PropertyRange {
        start: 0xB3C,
        end: 0xB3D,
        value: "NSM",
    },
    PropertyRange {
        start: 0xB3D,
        end: 0xB3F,
        value: "L",
    },
    PropertyRange {
        start: 0xB3F,
        end: 0xB40,
        value: "NSM",
    },
    PropertyRange {
        start: 0xB40,
        end: 0xB41,
        value: "L",
    },
    PropertyRange {
        start: 0xB41,
        end: 0xB45,
        value: "NSM",
    },
    PropertyRange {
        start: 0xB47,
        end: 0xB49,
        value: "L",
    },
    PropertyRange {
        start: 0xB4B,
        end: 0xB4D,
        value: "L",
    },
    PropertyRange {
        start: 0xB4D,
        end: 0xB4E,
        value: "NSM",
    },
    PropertyRange {
        start: 0xB55,
        end: 0xB57,
        value: "NSM",
    },
    PropertyRange {
        start: 0xB57,
        end: 0xB58,
        value: "L",
    },
    PropertyRange {
        start: 0xB5C,
        end: 0xB5E,
        value: "L",
    },
    PropertyRange {
        start: 0xB5F,
        end: 0xB62,
        value: "L",
    },
    PropertyRange {
        start: 0xB62,
        end: 0xB64,
        value: "NSM",
    },
    PropertyRange {
        start: 0xB66,
        end: 0xB78,
        value: "L",
    },
    PropertyRange {
        start: 0xB82,
        end: 0xB83,
        value: "NSM",
    },
    PropertyRange {
        start: 0xB83,
        end: 0xB84,
        value: "L",
    },
    PropertyRange {
        start: 0xB85,
        end: 0xB8B,
        value: "L",
    },
    PropertyRange {
        start: 0xB8E,
        end: 0xB91,
        value: "L",
    },
    PropertyRange {
        start: 0xB92,
        end: 0xB96,
        value: "L",
    },
    PropertyRange {
        start: 0xB99,
        end: 0xB9B,
        value: "L",
    },
    PropertyRange {
        start: 0xB9C,
        end: 0xB9D,
        value: "L",
    },
    PropertyRange {
        start: 0xB9E,
        end: 0xBA0,
        value: "L",
    },
    PropertyRange {
        start: 0xBA3,
        end: 0xBA5,
        value: "L",
    },
    PropertyRange {
        start: 0xBA8,
        end: 0xBAB,
        value: "L",
    },
    PropertyRange {
        start: 0xBAE,
        end: 0xBBA,
        value: "L",
    },
    PropertyRange {
        start: 0xBBE,
        end: 0xBC0,
        value: "L",
    },
    PropertyRange {
        start: 0xBC0,
        end: 0xBC1,
        value: "NSM",
    },
    PropertyRange {
        start: 0xBC1,
        end: 0xBC3,
        value: "L",
    },
    PropertyRange {
        start: 0xBC6,
        end: 0xBC9,
        value: "L",
    },
    PropertyRange {
        start: 0xBCA,
        end: 0xBCD,
        value: "L",
    },
    PropertyRange {
        start: 0xBCD,
        end: 0xBCE,
        value: "NSM",
    },
    PropertyRange {
        start: 0xBD0,
        end: 0xBD1,
        value: "L",
    },
    PropertyRange {
        start: 0xBD7,
        end: 0xBD8,
        value: "L",
    },
    PropertyRange {
        start: 0xBE6,
        end: 0xBF3,
        value: "L",
    },
    PropertyRange {
        start: 0xBF3,
        end: 0xBF9,
        value: "ON",
    },
    PropertyRange {
        start: 0xBF9,
        end: 0xBFA,
        value: "ET",
    },
    PropertyRange {
        start: 0xBFA,
        end: 0xBFB,
        value: "ON",
    },
    PropertyRange {
        start: 0xC00,
        end: 0xC01,
        value: "NSM",
    },
    PropertyRange {
        start: 0xC01,
        end: 0xC04,
        value: "L",
    },
    PropertyRange {
        start: 0xC04,
        end: 0xC05,
        value: "NSM",
    },
    PropertyRange {
        start: 0xC05,
        end: 0xC0D,
        value: "L",
    },
    PropertyRange {
        start: 0xC0E,
        end: 0xC11,
        value: "L",
    },
    PropertyRange {
        start: 0xC12,
        end: 0xC29,
        value: "L",
    },
    PropertyRange {
        start: 0xC2A,
        end: 0xC3A,
        value: "L",
    },
    PropertyRange {
        start: 0xC3C,
        end: 0xC3D,
        value: "NSM",
    },
    PropertyRange {
        start: 0xC3D,
        end: 0xC3E,
        value: "L",
    },
    PropertyRange {
        start: 0xC3E,
        end: 0xC41,
        value: "NSM",
    },
    PropertyRange {
        start: 0xC41,
        end: 0xC45,
        value: "L",
    },
    PropertyRange {
        start: 0xC46,
        end: 0xC49,
        value: "NSM",
    },
    PropertyRange {
        start: 0xC4A,
        end: 0xC4E,
        value: "NSM",
    },
    PropertyRange {
        start: 0xC55,
        end: 0xC57,
        value: "NSM",
    },
    PropertyRange {
        start: 0xC58,
        end: 0xC5B,
        value: "L",
    },
    PropertyRange {
        start: 0xC5D,
        end: 0xC5E,
        value: "L",
    },
    PropertyRange {
        start: 0xC60,
        end: 0xC62,
        value: "L",
    },
    PropertyRange {
        start: 0xC62,
        end: 0xC64,
        value: "NSM",
    },
    PropertyRange {
        start: 0xC66,
        end: 0xC70,
        value: "L",
    },
    PropertyRange {
        start: 0xC77,
        end: 0xC78,
        value: "L",
    },
    PropertyRange {
        start: 0xC78,
        end: 0xC7F,
        value: "ON",
    },
    PropertyRange {
        start: 0xC7F,
        end: 0xC81,
        value: "L",
    },
    PropertyRange {
        start: 0xC81,
        end: 0xC82,
        value: "NSM",
    },
    PropertyRange {
        start: 0xC82,
        end: 0xC8D,
        value: "L",
    },
    PropertyRange {
        start: 0xC8E,
        end: 0xC91,
        value: "L",
    },
    PropertyRange {
        start: 0xC92,
        end: 0xCA9,
        value: "L",
    },
    PropertyRange {
        start: 0xCAA,
        end: 0xCB4,
        value: "L",
    },
    PropertyRange {
        start: 0xCB5,
        end: 0xCBA,
        value: "L",
    },
    PropertyRange {
        start: 0xCBC,
        end: 0xCBD,
        value: "NSM",
    },
    PropertyRange {
        start: 0xCBD,
        end: 0xCC5,
        value: "L",
    },
    PropertyRange {
        start: 0xCC6,
        end: 0xCC9,
        value: "L",
    },
    PropertyRange {
        start: 0xCCA,
        end: 0xCCC,
        value: "L",
    },
    PropertyRange {
        start: 0xCCC,
        end: 0xCCE,
        value: "NSM",
    },
    PropertyRange {
        start: 0xCD5,
        end: 0xCD7,
        value: "L",
    },
    PropertyRange {
        start: 0xCDD,
        end: 0xCDF,
        value: "L",
    },
    PropertyRange {
        start: 0xCE0,
        end: 0xCE2,
        value: "L",
    },
    PropertyRange {
        start: 0xCE2,
        end: 0xCE4,
        value: "NSM",
    },
    PropertyRange {
        start: 0xCE6,
        end: 0xCF0,
        value: "L",
    },
    PropertyRange {
        start: 0xCF1,
        end: 0xCF4,
        value: "L",
    },
    PropertyRange {
        start: 0xD00,
        end: 0xD02,
        value: "NSM",
    },
    PropertyRange {
        start: 0xD02,
        end: 0xD0D,
        value: "L",
    },
    PropertyRange {
        start: 0xD0E,
        end: 0xD11,
        value: "L",
    },
    PropertyRange {
        start: 0xD12,
        end: 0xD3B,
        value: "L",
    },
    PropertyRange {
        start: 0xD3B,
        end: 0xD3D,
        value: "NSM",
    },
    PropertyRange {
        start: 0xD3D,
        end: 0xD41,
        value: "L",
    },
    PropertyRange {
        start: 0xD41,
        end: 0xD45,
        value: "NSM",
    },
    PropertyRange {
        start: 0xD46,
        end: 0xD49,
        value: "L",
    },
    PropertyRange {
        start: 0xD4A,
        end: 0xD4D,
        value: "L",
    },
    PropertyRange {
        start: 0xD4D,
        end: 0xD4E,
        value: "NSM",
    },
    PropertyRange {
        start: 0xD4E,
        end: 0xD50,
        value: "L",
    },
    PropertyRange {
        start: 0xD54,
        end: 0xD62,
        value: "L",
    },
    PropertyRange {
        start: 0xD62,
        end: 0xD64,
        value: "NSM",
    },
    PropertyRange {
        start: 0xD66,
        end: 0xD80,
        value: "L",
    },
    PropertyRange {
        start: 0xD81,
        end: 0xD82,
        value: "NSM",
    },
    PropertyRange {
        start: 0xD82,
        end: 0xD84,
        value: "L",
    },
    PropertyRange {
        start: 0xD85,
        end: 0xD97,
        value: "L",
    },
    PropertyRange {
        start: 0xD9A,
        end: 0xDB2,
        value: "L",
    },
    PropertyRange {
        start: 0xDB3,
        end: 0xDBC,
        value: "L",
    },
    PropertyRange {
        start: 0xDBD,
        end: 0xDBE,
        value: "L",
    },
    PropertyRange {
        start: 0xDC0,
        end: 0xDC7,
        value: "L",
    },
    PropertyRange {
        start: 0xDCA,
        end: 0xDCB,
        value: "NSM",
    },
    PropertyRange {
        start: 0xDCF,
        end: 0xDD2,
        value: "L",
    },
    PropertyRange {
        start: 0xDD2,
        end: 0xDD5,
        value: "NSM",
    },
    PropertyRange {
        start: 0xDD6,
        end: 0xDD7,
        value: "NSM",
    },
    PropertyRange {
        start: 0xDD8,
        end: 0xDE0,
        value: "L",
    },
    PropertyRange {
        start: 0xDE6,
        end: 0xDF0,
        value: "L",
    },
    PropertyRange {
        start: 0xDF2,
        end: 0xDF5,
        value: "L",
    },
    PropertyRange {
        start: 0xE01,
        end: 0xE31,
        value: "L",
    },
    PropertyRange {
        start: 0xE31,
        end: 0xE32,
        value: "NSM",
    },
    PropertyRange {
        start: 0xE32,
        end: 0xE34,
        value: "L",
    },
    PropertyRange {
        start: 0xE34,
        end: 0xE3B,
        value: "NSM",
    },
    PropertyRange {
        start: 0xE3F,
        end: 0xE40,
        value: "ET",
    },
    PropertyRange {
        start: 0xE40,
        end: 0xE47,
        value: "L",
    },
    PropertyRange {
        start: 0xE47,
        end: 0xE4F,
        value: "NSM",
    },
    PropertyRange {
        start: 0xE4F,
        end: 0xE5C,
        value: "L",
    },
    PropertyRange {
        start: 0xE81,
        end: 0xE83,
        value: "L",
    },
    PropertyRange {
        start: 0xE84,
        end: 0xE85,
        value: "L",
    },
    PropertyRange {
        start: 0xE86,
        end: 0xE8B,
        value: "L",
    },
    PropertyRange {
        start: 0xE8C,
        end: 0xEA4,
        value: "L",
    },
    PropertyRange {
        start: 0xEA5,
        end: 0xEA6,
        value: "L",
    },
    PropertyRange {
        start: 0xEA7,
        end: 0xEB1,
        value: "L",
    },
    PropertyRange {
        start: 0xEB1,
        end: 0xEB2,
        value: "NSM",
    },
    PropertyRange {
        start: 0xEB2,
        end: 0xEB4,
        value: "L",
    },
    PropertyRange {
        start: 0xEB4,
        end: 0xEBD,
        value: "NSM",
    },
    PropertyRange {
        start: 0xEBD,
        end: 0xEBE,
        value: "L",
    },
    PropertyRange {
        start: 0xEC0,
        end: 0xEC5,
        value: "L",
    },
    PropertyRange {
        start: 0xEC6,
        end: 0xEC7,
        value: "L",
    },
    PropertyRange {
        start: 0xEC8,
        end: 0xECF,
        value: "NSM",
    },
    PropertyRange {
        start: 0xED0,
        end: 0xEDA,
        value: "L",
    },
    PropertyRange {
        start: 0xEDC,
        end: 0xEE0,
        value: "L",
    },
    PropertyRange {
        start: 0xF00,
        end: 0xF18,
        value: "L",
    },
    PropertyRange {
        start: 0xF18,
        end: 0xF1A,
        value: "NSM",
    },
    PropertyRange {
        start: 0xF1A,
        end: 0xF35,
        value: "L",
    },
    PropertyRange {
        start: 0xF35,
        end: 0xF36,
        value: "NSM",
    },
    PropertyRange {
        start: 0xF36,
        end: 0xF37,
        value: "L",
    },
    PropertyRange {
        start: 0xF37,
        end: 0xF38,
        value: "NSM",
    },
    PropertyRange {
        start: 0xF38,
        end: 0xF39,
        value: "L",
    },
    PropertyRange {
        start: 0xF39,
        end: 0xF3A,
        value: "NSM",
    },
    PropertyRange {
        start: 0xF3A,
        end: 0xF3E,
        value: "ON",
    },
    PropertyRange {
        start: 0xF3E,
        end: 0xF48,
        value: "L",
    },
    PropertyRange {
        start: 0xF49,
        end: 0xF6D,
        value: "L",
    },
    PropertyRange {
        start: 0xF71,
        end: 0xF7F,
        value: "NSM",
    },
    PropertyRange {
        start: 0xF7F,
        end: 0xF80,
        value: "L",
    },
    PropertyRange {
        start: 0xF80,
        end: 0xF85,
        value: "NSM",
    },
    PropertyRange {
        start: 0xF85,
        end: 0xF86,
        value: "L",
    },
    PropertyRange {
        start: 0xF86,
        end: 0xF88,
        value: "NSM",
    },
    PropertyRange {
        start: 0xF88,
        end: 0xF8D,
        value: "L",
    },
    PropertyRange {
        start: 0xF8D,
        end: 0xF98,
        value: "NSM",
    },
    PropertyRange {
        start: 0xF99,
        end: 0xFBD,
        value: "NSM",
    },
    PropertyRange {
        start: 0xFBE,
        end: 0xFC6,
        value: "L",
    },
    PropertyRange {
        start: 0xFC6,
        end: 0xFC7,
        value: "NSM",
    },
    PropertyRange {
        start: 0xFC7,
        end: 0xFCD,
        value: "L",
    },
    PropertyRange {
        start: 0xFCE,
        end: 0xFDB,
        value: "L",
    },
    PropertyRange {
        start: 0x1000,
        end: 0x102D,
        value: "L",
    },
    PropertyRange {
        start: 0x102D,
        end: 0x1031,
        value: "NSM",
    },
    PropertyRange {
        start: 0x1031,
        end: 0x1032,
        value: "L",
    },
    PropertyRange {
        start: 0x1032,
        end: 0x1038,
        value: "NSM",
    },
    PropertyRange {
        start: 0x1038,
        end: 0x1039,
        value: "L",
    },
    PropertyRange {
        start: 0x1039,
        end: 0x103B,
        value: "NSM",
    },
    PropertyRange {
        start: 0x103B,
        end: 0x103D,
        value: "L",
    },
    PropertyRange {
        start: 0x103D,
        end: 0x103F,
        value: "NSM",
    },
    PropertyRange {
        start: 0x103F,
        end: 0x1058,
        value: "L",
    },
    PropertyRange {
        start: 0x1058,
        end: 0x105A,
        value: "NSM",
    },
    PropertyRange {
        start: 0x105A,
        end: 0x105E,
        value: "L",
    },
    PropertyRange {
        start: 0x105E,
        end: 0x1061,
        value: "NSM",
    },
    PropertyRange {
        start: 0x1061,
        end: 0x1071,
        value: "L",
    },
    PropertyRange {
        start: 0x1071,
        end: 0x1075,
        value: "NSM",
    },
    PropertyRange {
        start: 0x1075,
        end: 0x1082,
        value: "L",
    },
    PropertyRange {
        start: 0x1082,
        end: 0x1083,
        value: "NSM",
    },
    PropertyRange {
        start: 0x1083,
        end: 0x1085,
        value: "L",
    },
    PropertyRange {
        start: 0x1085,
        end: 0x1087,
        value: "NSM",
    },
    PropertyRange {
        start: 0x1087,
        end: 0x108D,
        value: "L",
    },
    PropertyRange {
        start: 0x108D,
        end: 0x108E,
        value: "NSM",
    },
    PropertyRange {
        start: 0x108E,
        end: 0x109D,
        value: "L",
    },
    PropertyRange {
        start: 0x109D,
        end: 0x109E,
        value: "NSM",
    },
    PropertyRange {
        start: 0x109E,
        end: 0x10C6,
        value: "L",
    },
    PropertyRange {
        start: 0x10C7,
        end: 0x10C8,
        value: "L",
    },
    PropertyRange {
        start: 0x10CD,
        end: 0x10CE,
        value: "L",
    },
    PropertyRange {
        start: 0x10D0,
        end: 0x1249,
        value: "L",
    },
    PropertyRange {
        start: 0x124A,
        end: 0x124E,
        value: "L",
    },
    PropertyRange {
        start: 0x1250,
        end: 0x1257,
        value: "L",
    },
    PropertyRange {
        start: 0x1258,
        end: 0x1259,
        value: "L",
    },
    PropertyRange {
        start: 0x125A,
        end: 0x125E,
        value: "L",
    },
    PropertyRange {
        start: 0x1260,
        end: 0x1289,
        value: "L",
    },
    PropertyRange {
        start: 0x128A,
        end: 0x128E,
        value: "L",
    },
    PropertyRange {
        start: 0x1290,
        end: 0x12B1,
        value: "L",
    },
    PropertyRange {
        start: 0x12B2,
        end: 0x12B6,
        value: "L",
    },
    PropertyRange {
        start: 0x12B8,
        end: 0x12BF,
        value: "L",
    },
    PropertyRange {
        start: 0x12C0,
        end: 0x12C1,
        value: "L",
    },
    PropertyRange {
        start: 0x12C2,
        end: 0x12C6,
        value: "L",
    },
    PropertyRange {
        start: 0x12C8,
        end: 0x12D7,
        value: "L",
    },
    PropertyRange {
        start: 0x12D8,
        end: 0x1311,
        value: "L",
    },
    PropertyRange {
        start: 0x1312,
        end: 0x1316,
        value: "L",
    },
    PropertyRange {
        start: 0x1318,
        end: 0x135B,
        value: "L",
    },
    PropertyRange {
        start: 0x135D,
        end: 0x1360,
        value: "NSM",
    },
    PropertyRange {
        start: 0x1360,
        end: 0x137D,
        value: "L",
    },
    PropertyRange {
        start: 0x1380,
        end: 0x1390,
        value: "L",
    },
    PropertyRange {
        start: 0x1390,
        end: 0x139A,
        value: "ON",
    },
    PropertyRange {
        start: 0x13A0,
        end: 0x13F6,
        value: "L",
    },
    PropertyRange {
        start: 0x13F8,
        end: 0x13FE,
        value: "L",
    },
    PropertyRange {
        start: 0x1400,
        end: 0x1401,
        value: "ON",
    },
    PropertyRange {
        start: 0x1401,
        end: 0x1680,
        value: "L",
    },
    PropertyRange {
        start: 0x1680,
        end: 0x1681,
        value: "WS",
    },
    PropertyRange {
        start: 0x1681,
        end: 0x169B,
        value: "L",
    },
    PropertyRange {
        start: 0x169B,
        end: 0x169D,
        value: "ON",
    },
    PropertyRange {
        start: 0x16A0,
        end: 0x16F9,
        value: "L",
    },
    PropertyRange {
        start: 0x1700,
        end: 0x1712,
        value: "L",
    },
    PropertyRange {
        start: 0x1712,
        end: 0x1715,
        value: "NSM",
    },
    PropertyRange {
        start: 0x1715,
        end: 0x1716,
        value: "L",
    },
    PropertyRange {
        start: 0x171F,
        end: 0x1732,
        value: "L",
    },
    PropertyRange {
        start: 0x1732,
        end: 0x1734,
        value: "NSM",
    },
    PropertyRange {
        start: 0x1734,
        end: 0x1737,
        value: "L",
    },
    PropertyRange {
        start: 0x1740,
        end: 0x1752,
        value: "L",
    },
    PropertyRange {
        start: 0x1752,
        end: 0x1754,
        value: "NSM",
    },
    PropertyRange {
        start: 0x1760,
        end: 0x176D,
        value: "L",
    },
    PropertyRange {
        start: 0x176E,
        end: 0x1771,
        value: "L",
    },
    PropertyRange {
        start: 0x1772,
        end: 0x1774,
        value: "NSM",
    },
    PropertyRange {
        start: 0x1780,
        end: 0x17B4,
        value: "L",
    },
    PropertyRange {
        start: 0x17B4,
        end: 0x17B6,
        value: "NSM",
    },
    PropertyRange {
        start: 0x17B6,
        end: 0x17B7,
        value: "L",
    },
    PropertyRange {
        start: 0x17B7,
        end: 0x17BE,
        value: "NSM",
    },
    PropertyRange {
        start: 0x17BE,
        end: 0x17C6,
        value: "L",
    },
    PropertyRange {
        start: 0x17C6,
        end: 0x17C7,
        value: "NSM",
    },
    PropertyRange {
        start: 0x17C7,
        end: 0x17C9,
        value: "L",
    },
    PropertyRange {
        start: 0x17C9,
        end: 0x17D4,
        value: "NSM",
    },
    PropertyRange {
        start: 0x17D4,
        end: 0x17DB,
        value: "L",
    },
    PropertyRange {
        start: 0x17DB,
        end: 0x17DC,
        value: "ET",
    },
    PropertyRange {
        start: 0x17DC,
        end: 0x17DD,
        value: "L",
    },
    PropertyRange {
        start: 0x17DD,
        end: 0x17DE,
        value: "NSM",
    },
    PropertyRange {
        start: 0x17E0,
        end: 0x17EA,
        value: "L",
    },
    PropertyRange {
        start: 0x17F0,
        end: 0x17FA,
        value: "ON",
    },
    PropertyRange {
        start: 0x1800,
        end: 0x180B,
        value: "ON",
    },
    PropertyRange {
        start: 0x180B,
        end: 0x180E,
        value: "NSM",
    },
    PropertyRange {
        start: 0x180E,
        end: 0x180F,
        value: "BN",
    },
    PropertyRange {
        start: 0x180F,
        end: 0x1810,
        value: "NSM",
    },
    PropertyRange {
        start: 0x1810,
        end: 0x181A,
        value: "L",
    },
    PropertyRange {
        start: 0x1820,
        end: 0x1879,
        value: "L",
    },
    PropertyRange {
        start: 0x1880,
        end: 0x1885,
        value: "L",
    },
    PropertyRange {
        start: 0x1885,
        end: 0x1887,
        value: "NSM",
    },
    PropertyRange {
        start: 0x1887,
        end: 0x18A9,
        value: "L",
    },
    PropertyRange {
        start: 0x18A9,
        end: 0x18AA,
        value: "NSM",
    },
    PropertyRange {
        start: 0x18AA,
        end: 0x18AB,
        value: "L",
    },
    PropertyRange {
        start: 0x18B0,
        end: 0x18F6,
        value: "L",
    },
    PropertyRange {
        start: 0x1900,
        end: 0x191F,
        value: "L",
    },
    PropertyRange {
        start: 0x1920,
        end: 0x1923,
        value: "NSM",
    },
    PropertyRange {
        start: 0x1923,
        end: 0x1927,
        value: "L",
    },
    PropertyRange {
        start: 0x1927,
        end: 0x1929,
        value: "NSM",
    },
    PropertyRange {
        start: 0x1929,
        end: 0x192C,
        value: "L",
    },
    PropertyRange {
        start: 0x1930,
        end: 0x1932,
        value: "L",
    },
    PropertyRange {
        start: 0x1932,
        end: 0x1933,
        value: "NSM",
    },
    PropertyRange {
        start: 0x1933,
        end: 0x1939,
        value: "L",
    },
    PropertyRange {
        start: 0x1939,
        end: 0x193C,
        value: "NSM",
    },
    PropertyRange {
        start: 0x1940,
        end: 0x1941,
        value: "ON",
    },
    PropertyRange {
        start: 0x1944,
        end: 0x1946,
        value: "ON",
    },
    PropertyRange {
        start: 0x1946,
        end: 0x196E,
        value: "L",
    },
    PropertyRange {
        start: 0x1970,
        end: 0x1975,
        value: "L",
    },
    PropertyRange {
        start: 0x1980,
        end: 0x19AC,
        value: "L",
    },
    PropertyRange {
        start: 0x19B0,
        end: 0x19CA,
        value: "L",
    },
    PropertyRange {
        start: 0x19D0,
        end: 0x19DB,
        value: "L",
    },
    PropertyRange {
        start: 0x19DE,
        end: 0x1A00,
        value: "ON",
    },
    PropertyRange {
        start: 0x1A00,
        end: 0x1A17,
        value: "L",
    },
    PropertyRange {
        start: 0x1A17,
        end: 0x1A19,
        value: "NSM",
    },
    PropertyRange {
        start: 0x1A19,
        end: 0x1A1B,
        value: "L",
    },
    PropertyRange {
        start: 0x1A1B,
        end: 0x1A1C,
        value: "NSM",
    },
    PropertyRange {
        start: 0x1A1E,
        end: 0x1A56,
        value: "L",
    },
    PropertyRange {
        start: 0x1A56,
        end: 0x1A57,
        value: "NSM",
    },
    PropertyRange {
        start: 0x1A57,
        end: 0x1A58,
        value: "L",
    },
    PropertyRange {
        start: 0x1A58,
        end: 0x1A5F,
        value: "NSM",
    },
    PropertyRange {
        start: 0x1A60,
        end: 0x1A61,
        value: "NSM",
    },
    PropertyRange {
        start: 0x1A61,
        end: 0x1A62,
        value: "L",
    },
    PropertyRange {
        start: 0x1A62,
        end: 0x1A63,
        value: "NSM",
    },
    PropertyRange {
        start: 0x1A63,
        end: 0x1A65,
        value: "L",
    },
    PropertyRange {
        start: 0x1A65,
        end: 0x1A6D,
        value: "NSM",
    },
    PropertyRange {
        start: 0x1A6D,
        end: 0x1A73,
        value: "L",
    },
    PropertyRange {
        start: 0x1A73,
        end: 0x1A7D,
        value: "NSM",
    },
    PropertyRange {
        start: 0x1A7F,
        end: 0x1A80,
        value: "NSM",
    },
    PropertyRange {
        start: 0x1A80,
        end: 0x1A8A,
        value: "L",
    },
    PropertyRange {
        start: 0x1A90,
        end: 0x1A9A,
        value: "L",
    },
    PropertyRange {
        start: 0x1AA0,
        end: 0x1AAE,
        value: "L",
    },
    PropertyRange {
        start: 0x1AB0,
        end: 0x1ACF,
        value: "NSM",
    },
    PropertyRange {
        start: 0x1B00,
        end: 0x1B04,
        value: "NSM",
    },
    PropertyRange {
        start: 0x1B04,
        end: 0x1B34,
        value: "L",
    },
    PropertyRange {
        start: 0x1B34,
        end: 0x1B35,
        value: "NSM",
    },
    PropertyRange {
        start: 0x1B35,
        end: 0x1B36,
        value: "L",
    },
    PropertyRange {
        start: 0x1B36,
        end: 0x1B3B,
        value: "NSM",
    },
    PropertyRange {
        start: 0x1B3B,
        end: 0x1B3C,
        value: "L",
    },
    PropertyRange {
        start: 0x1B3C,
        end: 0x1B3D,
        value: "NSM",
    },
    PropertyRange {
        start: 0x1B3D,
        end: 0x1B42,
        value: "L",
    },
    PropertyRange {
        start: 0x1B42,
        end: 0x1B43,
        value: "NSM",
    },
    PropertyRange {
        start: 0x1B43,
        end: 0x1B4D,
        value: "L",
    },
    PropertyRange {
        start: 0x1B4E,
        end: 0x1B6B,
        value: "L",
    },
    PropertyRange {
        start: 0x1B6B,
        end: 0x1B74,
        value: "NSM",
    },
    PropertyRange {
        start: 0x1B74,
        end: 0x1B80,
        value: "L",
    },
    PropertyRange {
        start: 0x1B80,
        end: 0x1B82,
        value: "NSM",
    },
    PropertyRange {
        start: 0x1B82,
        end: 0x1BA2,
        value: "L",
    },
    PropertyRange {
        start: 0x1BA2,
        end: 0x1BA6,
        value: "NSM",
    },
    PropertyRange {
        start: 0x1BA6,
        end: 0x1BA8,
        value: "L",
    },
    PropertyRange {
        start: 0x1BA8,
        end: 0x1BAA,
        value: "NSM",
    },
    PropertyRange {
        start: 0x1BAA,
        end: 0x1BAB,
        value: "L",
    },
    PropertyRange {
        start: 0x1BAB,
        end: 0x1BAE,
        value: "NSM",
    },
    PropertyRange {
        start: 0x1BAE,
        end: 0x1BE6,
        value: "L",
    },
    PropertyRange {
        start: 0x1BE6,
        end: 0x1BE7,
        value: "NSM",
    },
    PropertyRange {
        start: 0x1BE7,
        end: 0x1BE8,
        value: "L",
    },
    PropertyRange {
        start: 0x1BE8,
        end: 0x1BEA,
        value: "NSM",
    },
    PropertyRange {
        start: 0x1BEA,
        end: 0x1BED,
        value: "L",
    },
    PropertyRange {
        start: 0x1BED,
        end: 0x1BEE,
        value: "NSM",
    },
    PropertyRange {
        start: 0x1BEE,
        end: 0x1BEF,
        value: "L",
    },
    PropertyRange {
        start: 0x1BEF,
        end: 0x1BF2,
        value: "NSM",
    },
    PropertyRange {
        start: 0x1BF2,
        end: 0x1BF4,
        value: "L",
    },
    PropertyRange {
        start: 0x1BFC,
        end: 0x1C2C,
        value: "L",
    },
    PropertyRange {
        start: 0x1C2C,
        end: 0x1C34,
        value: "NSM",
    },
    PropertyRange {
        start: 0x1C34,
        end: 0x1C36,
        value: "L",
    },
    PropertyRange {
        start: 0x1C36,
        end: 0x1C38,
        value: "NSM",
    },
    PropertyRange {
        start: 0x1C3B,
        end: 0x1C4A,
        value: "L",
    },
    PropertyRange {
        start: 0x1C4D,
        end: 0x1C8B,
        value: "L",
    },
    PropertyRange {
        start: 0x1C90,
        end: 0x1CBB,
        value: "L",
    },
    PropertyRange {
        start: 0x1CBD,
        end: 0x1CC8,
        value: "L",
    },
    PropertyRange {
        start: 0x1CD0,
        end: 0x1CD3,
        value: "NSM",
    },
    PropertyRange {
        start: 0x1CD3,
        end: 0x1CD4,
        value: "L",
    },
    PropertyRange {
        start: 0x1CD4,
        end: 0x1CE1,
        value: "NSM",
    },
    PropertyRange {
        start: 0x1CE1,
        end: 0x1CE2,
        value: "L",
    },
    PropertyRange {
        start: 0x1CE2,
        end: 0x1CE9,
        value: "NSM",
    },
    PropertyRange {
        start: 0x1CE9,
        end: 0x1CED,
        value: "L",
    },
    PropertyRange {
        start: 0x1CED,
        end: 0x1CEE,
        value: "NSM",
    },
    PropertyRange {
        start: 0x1CEE,
        end: 0x1CF4,
        value: "L",
    },
    PropertyRange {
        start: 0x1CF4,
        end: 0x1CF5,
        value: "NSM",
    },
    PropertyRange {
        start: 0x1CF5,
        end: 0x1CF8,
        value: "L",
    },
    PropertyRange {
        start: 0x1CF8,
        end: 0x1CFA,
        value: "NSM",
    },
    PropertyRange {
        start: 0x1CFA,
        end: 0x1CFB,
        value: "L",
    },
    PropertyRange {
        start: 0x1D00,
        end: 0x1DC0,
        value: "L",
    },
    PropertyRange {
        start: 0x1DC0,
        end: 0x1E00,
        value: "NSM",
    },
    PropertyRange {
        start: 0x1E00,
        end: 0x1F16,
        value: "L",
    },
    PropertyRange {
        start: 0x1F18,
        end: 0x1F1E,
        value: "L",
    },
    PropertyRange {
        start: 0x1F20,
        end: 0x1F46,
        value: "L",
    },
    PropertyRange {
        start: 0x1F48,
        end: 0x1F4E,
        value: "L",
    },
    PropertyRange {
        start: 0x1F50,
        end: 0x1F58,
        value: "L",
    },
    PropertyRange {
        start: 0x1F59,
        end: 0x1F5A,
        value: "L",
    },
    PropertyRange {
        start: 0x1F5B,
        end: 0x1F5C,
        value: "L",
    },
    PropertyRange {
        start: 0x1F5D,
        end: 0x1F5E,
        value: "L",
    },
    PropertyRange {
        start: 0x1F5F,
        end: 0x1F7E,
        value: "L",
    },
    PropertyRange {
        start: 0x1F80,
        end: 0x1FB5,
        value: "L",
    },
    PropertyRange {
        start: 0x1FB6,
        end: 0x1FBD,
        value: "L",
    },
    PropertyRange {
        start: 0x1FBD,
        end: 0x1FBE,
        value: "ON",
    },
    PropertyRange {
        start: 0x1FBE,
        end: 0x1FBF,
        value: "L",
    },
    PropertyRange {
        start: 0x1FBF,
        end: 0x1FC2,
        value: "ON",
    },
    PropertyRange {
        start: 0x1FC2,
        end: 0x1FC5,
        value: "L",
    },
    PropertyRange {
        start: 0x1FC6,
        end: 0x1FCD,
        value: "L",
    },
    PropertyRange {
        start: 0x1FCD,
        end: 0x1FD0,
        value: "ON",
    },
    PropertyRange {
        start: 0x1FD0,
        end: 0x1FD4,
        value: "L",
    },
    PropertyRange {
        start: 0x1FD6,
        end: 0x1FDC,
        value: "L",
    },
    PropertyRange {
        start: 0x1FDD,
        end: 0x1FE0,
        value: "ON",
    },
    PropertyRange {
        start: 0x1FE0,
        end: 0x1FED,
        value: "L",
    },
    PropertyRange {
        start: 0x1FED,
        end: 0x1FF0,
        value: "ON",
    },
    PropertyRange {
        start: 0x1FF2,
        end: 0x1FF5,
        value: "L",
    },
    PropertyRange {
        start: 0x1FF6,
        end: 0x1FFD,
        value: "L",
    },
    PropertyRange {
        start: 0x1FFD,
        end: 0x1FFF,
        value: "ON",
    },
    PropertyRange {
        start: 0x2000,
        end: 0x200B,
        value: "WS",
    },
    PropertyRange {
        start: 0x200B,
        end: 0x200E,
        value: "BN",
    },
    PropertyRange {
        start: 0x200E,
        end: 0x200F,
        value: "L",
    },
    PropertyRange {
        start: 0x200F,
        end: 0x2010,
        value: "R",
    },
    PropertyRange {
        start: 0x2010,
        end: 0x2028,
        value: "ON",
    },
    PropertyRange {
        start: 0x2028,
        end: 0x2029,
        value: "WS",
    },
    PropertyRange {
        start: 0x2029,
        end: 0x202A,
        value: "B",
    },
    PropertyRange {
        start: 0x202A,
        end: 0x202B,
        value: "LRE",
    },
    PropertyRange {
        start: 0x202B,
        end: 0x202C,
        value: "RLE",
    },
    PropertyRange {
        start: 0x202C,
        end: 0x202D,
        value: "PDF",
    },
    PropertyRange {
        start: 0x202D,
        end: 0x202E,
        value: "LRO",
    },
    PropertyRange {
        start: 0x202E,
        end: 0x202F,
        value: "RLO",
    },
    PropertyRange {
        start: 0x202F,
        end: 0x2030,
        value: "CS",
    },
    PropertyRange {
        start: 0x2030,
        end: 0x2035,
        value: "ET",
    },
    PropertyRange {
        start: 0x2035,
        end: 0x2044,
        value: "ON",
    },
    PropertyRange {
        start: 0x2044,
        end: 0x2045,
        value: "CS",
    },
    PropertyRange {
        start: 0x2045,
        end: 0x205F,
        value: "ON",
    },
    PropertyRange {
        start: 0x205F,
        end: 0x2060,
        value: "WS",
    },
    PropertyRange {
        start: 0x2060,
        end: 0x2065,
        value: "BN",
    },
    PropertyRange {
        start: 0x2066,
        end: 0x2067,
        value: "LRI",
    },
    PropertyRange {
        start: 0x2067,
        end: 0x2068,
        value: "RLI",
    },
    PropertyRange {
        start: 0x2068,
        end: 0x2069,
        value: "FSI",
    },
    PropertyRange {
        start: 0x2069,
        end: 0x206A,
        value: "PDI",
    },
    PropertyRange {
        start: 0x206A,
        end: 0x2070,
        value: "BN",
    },
    PropertyRange {
        start: 0x2070,
        end: 0x2071,
        value: "EN",
    },
    PropertyRange {
        start: 0x2071,
        end: 0x2072,
        value: "L",
    },
    PropertyRange {
        start: 0x2074,
        end: 0x207A,
        value: "EN",
    },
    PropertyRange {
        start: 0x207A,
        end: 0x207C,
        value: "ES",
    },
    PropertyRange {
        start: 0x207C,
        end: 0x207F,
        value: "ON",
    },
    PropertyRange {
        start: 0x207F,
        end: 0x2080,
        value: "L",
    },
    PropertyRange {
        start: 0x2080,
        end: 0x208A,
        value: "EN",
    },
    PropertyRange {
        start: 0x208A,
        end: 0x208C,
        value: "ES",
    },
    PropertyRange {
        start: 0x208C,
        end: 0x208F,
        value: "ON",
    },
    PropertyRange {
        start: 0x2090,
        end: 0x209D,
        value: "L",
    },
    PropertyRange {
        start: 0x20A0,
        end: 0x20C1,
        value: "ET",
    },
    PropertyRange {
        start: 0x20D0,
        end: 0x20F1,
        value: "NSM",
    },
    PropertyRange {
        start: 0x2100,
        end: 0x2102,
        value: "ON",
    },
    PropertyRange {
        start: 0x2102,
        end: 0x2103,
        value: "L",
    },
    PropertyRange {
        start: 0x2103,
        end: 0x2107,
        value: "ON",
    },
    PropertyRange {
        start: 0x2107,
        end: 0x2108,
        value: "L",
    },
    PropertyRange {
        start: 0x2108,
        end: 0x210A,
        value: "ON",
    },
    PropertyRange {
        start: 0x210A,
        end: 0x2114,
        value: "L",
    },
    PropertyRange {
        start: 0x2114,
        end: 0x2115,
        value: "ON",
    },
    PropertyRange {
        start: 0x2115,
        end: 0x2116,
        value: "L",
    },
    PropertyRange {
        start: 0x2116,
        end: 0x2119,
        value: "ON",
    },
    PropertyRange {
        start: 0x2119,
        end: 0x211E,
        value: "L",
    },
    PropertyRange {
        start: 0x211E,
        end: 0x2124,
        value: "ON",
    },
    PropertyRange {
        start: 0x2124,
        end: 0x2125,
        value: "L",
    },
    PropertyRange {
        start: 0x2125,
        end: 0x2126,
        value: "ON",
    },
    PropertyRange {
        start: 0x2126,
        end: 0x2127,
        value: "L",
    },
    PropertyRange {
        start: 0x2127,
        end: 0x2128,
        value: "ON",
    },
    PropertyRange {
        start: 0x2128,
        end: 0x2129,
        value: "L",
    },
    PropertyRange {
        start: 0x2129,
        end: 0x212A,
        value: "ON",
    },
    PropertyRange {
        start: 0x212A,
        end: 0x212E,
        value: "L",
    },
    PropertyRange {
        start: 0x212E,
        end: 0x212F,
        value: "ET",
    },
    PropertyRange {
        start: 0x212F,
        end: 0x213A,
        value: "L",
    },
    PropertyRange {
        start: 0x213A,
        end: 0x213C,
        value: "ON",
    },
    PropertyRange {
        start: 0x213C,
        end: 0x2140,
        value: "L",
    },
    PropertyRange {
        start: 0x2140,
        end: 0x2145,
        value: "ON",
    },
    PropertyRange {
        start: 0x2145,
        end: 0x214A,
        value: "L",
    },
    PropertyRange {
        start: 0x214A,
        end: 0x214E,
        value: "ON",
    },
    PropertyRange {
        start: 0x214E,
        end: 0x2150,
        value: "L",
    },
    PropertyRange {
        start: 0x2150,
        end: 0x2160,
        value: "ON",
    },
    PropertyRange {
        start: 0x2160,
        end: 0x2189,
        value: "L",
    },
    PropertyRange {
        start: 0x2189,
        end: 0x218C,
        value: "ON",
    },
    PropertyRange {
        start: 0x2190,
        end: 0x2212,
        value: "ON",
    },
    PropertyRange {
        start: 0x2212,
        end: 0x2213,
        value: "ES",
    },
    PropertyRange {
        start: 0x2213,
        end: 0x2214,
        value: "ET",
    },
    PropertyRange {
        start: 0x2214,
        end: 0x2336,
        value: "ON",
    },
    PropertyRange {
        start: 0x2336,
        end: 0x237B,
        value: "L",
    },
    PropertyRange {
        start: 0x237B,
        end: 0x2395,
        value: "ON",
    },
    PropertyRange {
        start: 0x2395,
        end: 0x2396,
        value: "L",
    },
    PropertyRange {
        start: 0x2396,
        end: 0x242A,
        value: "ON",
    },
    PropertyRange {
        start: 0x2440,
        end: 0x244B,
        value: "ON",
    },
    PropertyRange {
        start: 0x2460,
        end: 0x2488,
        value: "ON",
    },
    PropertyRange {
        start: 0x2488,
        end: 0x249C,
        value: "EN",
    },
    PropertyRange {
        start: 0x249C,
        end: 0x24EA,
        value: "L",
    },
    PropertyRange {
        start: 0x24EA,
        end: 0x26AC,
        value: "ON",
    },
    PropertyRange {
        start: 0x26AC,
        end: 0x26AD,
        value: "L",
    },
    PropertyRange {
        start: 0x26AD,
        end: 0x2800,
        value: "ON",
    },
    PropertyRange {
        start: 0x2800,
        end: 0x2900,
        value: "L",
    },
    PropertyRange {
        start: 0x2900,
        end: 0x2B74,
        value: "ON",
    },
    PropertyRange {
        start: 0x2B76,
        end: 0x2B96,
        value: "ON",
    },
    PropertyRange {
        start: 0x2B97,
        end: 0x2C00,
        value: "ON",
    },
    PropertyRange {
        start: 0x2C00,
        end: 0x2CE5,
        value: "L",
    },
    PropertyRange {
        start: 0x2CE5,
        end: 0x2CEB,
        value: "ON",
    },
    PropertyRange {
        start: 0x2CEB,
        end: 0x2CEF,
        value: "L",
    },
    PropertyRange {
        start: 0x2CEF,
        end: 0x2CF2,
        value: "NSM",
    },
    PropertyRange {
        start: 0x2CF2,
        end: 0x2CF4,
        value: "L",
    },
    PropertyRange {
        start: 0x2CF9,
        end: 0x2D00,
        value: "ON",
    },
    PropertyRange {
        start: 0x2D00,
        end: 0x2D26,
        value: "L",
    },
    PropertyRange {
        start: 0x2D27,
        end: 0x2D28,
        value: "L",
    },
    PropertyRange {
        start: 0x2D2D,
        end: 0x2D2E,
        value: "L",
    },
    PropertyRange {
        start: 0x2D30,
        end: 0x2D68,
        value: "L",
    },
    PropertyRange {
        start: 0x2D6F,
        end: 0x2D71,
        value: "L",
    },
    PropertyRange {
        start: 0x2D7F,
        end: 0x2D80,
        value: "NSM",
    },
    PropertyRange {
        start: 0x2D80,
        end: 0x2D97,
        value: "L",
    },
    PropertyRange {
        start: 0x2DA0,
        end: 0x2DA7,
        value: "L",
    },
    PropertyRange {
        start: 0x2DA8,
        end: 0x2DAF,
        value: "L",
    },
    PropertyRange {
        start: 0x2DB0,
        end: 0x2DB7,
        value: "L",
    },
    PropertyRange {
        start: 0x2DB8,
        end: 0x2DBF,
        value: "L",
    },
    PropertyRange {
        start: 0x2DC0,
        end: 0x2DC7,
        value: "L",
    },
    PropertyRange {
        start: 0x2DC8,
        end: 0x2DCF,
        value: "L",
    },
    PropertyRange {
        start: 0x2DD0,
        end: 0x2DD7,
        value: "L",
    },
    PropertyRange {
        start: 0x2DD8,
        end: 0x2DDF,
        value: "L",
    },
    PropertyRange {
        start: 0x2DE0,
        end: 0x2E00,
        value: "NSM",
    },
    PropertyRange {
        start: 0x2E00,
        end: 0x2E5E,
        value: "ON",
    },
    PropertyRange {
        start: 0x2E80,
        end: 0x2E9A,
        value: "ON",
    },
    PropertyRange {
        start: 0x2E9B,
        end: 0x2EF4,
        value: "ON",
    },
    PropertyRange {
        start: 0x2F00,
        end: 0x2FD6,
        value: "ON",
    },
    PropertyRange {
        start: 0x2FF0,
        end: 0x3000,
        value: "ON",
    },
    PropertyRange {
        start: 0x3000,
        end: 0x3001,
        value: "WS",
    },
    PropertyRange {
        start: 0x3001,
        end: 0x3005,
        value: "ON",
    },
    PropertyRange {
        start: 0x3005,
        end: 0x3008,
        value: "L",
    },
    PropertyRange {
        start: 0x3008,
        end: 0x3021,
        value: "ON",
    },
    PropertyRange {
        start: 0x3021,
        end: 0x302A,
        value: "L",
    },
    PropertyRange {
        start: 0x302A,
        end: 0x302E,
        value: "NSM",
    },
    PropertyRange {
        start: 0x302E,
        end: 0x3030,
        value: "L",
    },
    PropertyRange {
        start: 0x3030,
        end: 0x3031,
        value: "ON",
    },
    PropertyRange {
        start: 0x3031,
        end: 0x3036,
        value: "L",
    },
    PropertyRange {
        start: 0x3036,
        end: 0x3038,
        value: "ON",
    },
    PropertyRange {
        start: 0x3038,
        end: 0x303D,
        value: "L",
    },
    PropertyRange {
        start: 0x303D,
        end: 0x3040,
        value: "ON",
    },
    PropertyRange {
        start: 0x3041,
        end: 0x3097,
        value: "L",
    },
    PropertyRange {
        start: 0x3099,
        end: 0x309B,
        value: "NSM",
    },
    PropertyRange {
        start: 0x309B,
        end: 0x309D,
        value: "ON",
    },
    PropertyRange {
        start: 0x309D,
        end: 0x30A0,
        value: "L",
    },
    PropertyRange {
        start: 0x30A0,
        end: 0x30A1,
        value: "ON",
    },
    PropertyRange {
        start: 0x30A1,
        end: 0x30FB,
        value: "L",
    },
    PropertyRange {
        start: 0x30FB,
        end: 0x30FC,
        value: "ON",
    },
    PropertyRange {
        start: 0x30FC,
        end: 0x3100,
        value: "L",
    },
    PropertyRange {
        start: 0x3105,
        end: 0x3130,
        value: "L",
    },
    PropertyRange {
        start: 0x3131,
        end: 0x318F,
        value: "L",
    },
    PropertyRange {
        start: 0x3190,
        end: 0x31C0,
        value: "L",
    },
    PropertyRange {
        start: 0x31C0,
        end: 0x31E6,
        value: "ON",
    },
    PropertyRange {
        start: 0x31EF,
        end: 0x31F0,
        value: "ON",
    },
    PropertyRange {
        start: 0x31F0,
        end: 0x321D,
        value: "L",
    },
    PropertyRange {
        start: 0x321D,
        end: 0x321F,
        value: "ON",
    },
    PropertyRange {
        start: 0x3220,
        end: 0x3250,
        value: "L",
    },
    PropertyRange {
        start: 0x3250,
        end: 0x3260,
        value: "ON",
    },
    PropertyRange {
        start: 0x3260,
        end: 0x327C,
        value: "L",
    },
    PropertyRange {
        start: 0x327C,
        end: 0x327F,
        value: "ON",
    },
    PropertyRange {
        start: 0x327F,
        end: 0x32B1,
        value: "L",
    },
    PropertyRange {
        start: 0x32B1,
        end: 0x32C0,
        value: "ON",
    },
    PropertyRange {
        start: 0x32C0,
        end: 0x32CC,
        value: "L",
    },
    PropertyRange {
        start: 0x32CC,
        end: 0x32D0,
        value: "ON",
    },
    PropertyRange {
        start: 0x32D0,
        end: 0x3377,
        value: "L",
    },
    PropertyRange {
        start: 0x3377,
        end: 0x337B,
        value: "ON",
    },
    PropertyRange {
        start: 0x337B,
        end: 0x33DE,
        value: "L",
    },
    PropertyRange {
        start: 0x33DE,
        end: 0x33E0,
        value: "ON",
    },
    PropertyRange {
        start: 0x33E0,
        end: 0x33FF,
        value: "L",
    },
    PropertyRange {
        start: 0x33FF,
        end: 0x3400,
        value: "ON",
    },
    PropertyRange {
        start: 0x3400,
        end: 0x4DC0,
        value: "L",
    },
    PropertyRange {
        start: 0x4DC0,
        end: 0x4E00,
        value: "ON",
    },
    PropertyRange {
        start: 0x4E00,
        end: 0xA48D,
        value: "L",
    },
    PropertyRange {
        start: 0xA490,
        end: 0xA4C7,
        value: "ON",
    },
    PropertyRange {
        start: 0xA4D0,
        end: 0xA60D,
        value: "L",
    },
    PropertyRange {
        start: 0xA60D,
        end: 0xA610,
        value: "ON",
    },
    PropertyRange {
        start: 0xA610,
        end: 0xA62C,
        value: "L",
    },
    PropertyRange {
        start: 0xA640,
        end: 0xA66F,
        value: "L",
    },
    PropertyRange {
        start: 0xA66F,
        end: 0xA673,
        value: "NSM",
    },
    PropertyRange {
        start: 0xA673,
        end: 0xA674,
        value: "ON",
    },
    PropertyRange {
        start: 0xA674,
        end: 0xA67E,
        value: "NSM",
    },
    PropertyRange {
        start: 0xA67E,
        end: 0xA680,
        value: "ON",
    },
    PropertyRange {
        start: 0xA680,
        end: 0xA69E,
        value: "L",
    },
    PropertyRange {
        start: 0xA69E,
        end: 0xA6A0,
        value: "NSM",
    },
    PropertyRange {
        start: 0xA6A0,
        end: 0xA6F0,
        value: "L",
    },
    PropertyRange {
        start: 0xA6F0,
        end: 0xA6F2,
        value: "NSM",
    },
    PropertyRange {
        start: 0xA6F2,
        end: 0xA6F8,
        value: "L",
    },
    PropertyRange {
        start: 0xA700,
        end: 0xA722,
        value: "ON",
    },
    PropertyRange {
        start: 0xA722,
        end: 0xA788,
        value: "L",
    },
    PropertyRange {
        start: 0xA788,
        end: 0xA789,
        value: "ON",
    },
    PropertyRange {
        start: 0xA789,
        end: 0xA7CE,
        value: "L",
    },
    PropertyRange {
        start: 0xA7D0,
        end: 0xA7D2,
        value: "L",
    },
    PropertyRange {
        start: 0xA7D3,
        end: 0xA7D4,
        value: "L",
    },
    PropertyRange {
        start: 0xA7D5,
        end: 0xA7DD,
        value: "L",
    },
    PropertyRange {
        start: 0xA7F2,
        end: 0xA802,
        value: "L",
    },
    PropertyRange {
        start: 0xA802,
        end: 0xA803,
        value: "NSM",
    },
    PropertyRange {
        start: 0xA803,
        end: 0xA806,
        value: "L",
    },
    PropertyRange {
        start: 0xA806,
        end: 0xA807,
        value: "NSM",
    },
    PropertyRange {
        start: 0xA807,
        end: 0xA80B,
        value: "L",
    },
    PropertyRange {
        start: 0xA80B,
        end: 0xA80C,
        value: "NSM",
    },
    PropertyRange {
        start: 0xA80C,
        end: 0xA825,
        value: "L",
    },
    PropertyRange {
        start: 0xA825,
        end: 0xA827,
        value: "NSM",
    },
    PropertyRange {
        start: 0xA827,
        end: 0xA828,
        value: "L",
    },
    PropertyRange {
        start: 0xA828,
        end: 0xA82C,
        value: "ON",
    },
    PropertyRange {
        start: 0xA82C,
        end: 0xA82D,
        value: "NSM",
    },
    PropertyRange {
        start: 0xA830,
        end: 0xA838,
        value: "L",
    },
    PropertyRange {
        start: 0xA838,
        end: 0xA83A,
        value: "ET",
    },
    PropertyRange {
        start: 0xA840,
        end: 0xA874,
        value: "L",
    },
    PropertyRange {
        start: 0xA874,
        end: 0xA878,
        value: "ON",
    },
    PropertyRange {
        start: 0xA880,
        end: 0xA8C4,
        value: "L",
    },
    PropertyRange {
        start: 0xA8C4,
        end: 0xA8C6,
        value: "NSM",
    },
    PropertyRange {
        start: 0xA8CE,
        end: 0xA8DA,
        value: "L",
    },
    PropertyRange {
        start: 0xA8E0,
        end: 0xA8F2,
        value: "NSM",
    },
    PropertyRange {
        start: 0xA8F2,
        end: 0xA8FF,
        value: "L",
    },
    PropertyRange {
        start: 0xA8FF,
        end: 0xA900,
        value: "NSM",
    },
    PropertyRange {
        start: 0xA900,
        end: 0xA926,
        value: "L",
    },
    PropertyRange {
        start: 0xA926,
        end: 0xA92E,
        value: "NSM",
    },
    PropertyRange {
        start: 0xA92E,
        end: 0xA947,
        value: "L",
    },
    PropertyRange {
        start: 0xA947,
        end: 0xA952,
        value: "NSM",
    },
    PropertyRange {
        start: 0xA952,
        end: 0xA954,
        value: "L",
    },
    PropertyRange {
        start: 0xA95F,
        end: 0xA97D,
        value: "L",
    },
    PropertyRange {
        start: 0xA980,
        end: 0xA983,
        value: "NSM",
    },
    PropertyRange {
        start: 0xA983,
        end: 0xA9B3,
        value: "L",
    },
    PropertyRange {
        start: 0xA9B3,
        end: 0xA9B4,
        value: "NSM",
    },
    PropertyRange {
        start: 0xA9B4,
        end: 0xA9B6,
        value: "L",
    },
    PropertyRange {
        start: 0xA9B6,
        end: 0xA9BA,
        value: "NSM",
    },
    PropertyRange {
        start: 0xA9BA,
        end: 0xA9BC,
        value: "L",
    },
    PropertyRange {
        start: 0xA9BC,
        end: 0xA9BE,
        value: "NSM",
    },
    PropertyRange {
        start: 0xA9BE,
        end: 0xA9CE,
        value: "L",
    },
    PropertyRange {
        start: 0xA9CF,
        end: 0xA9DA,
        value: "L",
    },
    PropertyRange {
        start: 0xA9DE,
        end: 0xA9E5,
        value: "L",
    },
    PropertyRange {
        start: 0xA9E5,
        end: 0xA9E6,
        value: "NSM",
    },
    PropertyRange {
        start: 0xA9E6,
        end: 0xA9FF,
        value: "L",
    },
    PropertyRange {
        start: 0xAA00,
        end: 0xAA29,
        value: "L",
    },
    PropertyRange {
        start: 0xAA29,
        end: 0xAA2F,
        value: "NSM",
    },
    PropertyRange {
        start: 0xAA2F,
        end: 0xAA31,
        value: "L",
    },
    PropertyRange {
        start: 0xAA31,
        end: 0xAA33,
        value: "NSM",
    },
    PropertyRange {
        start: 0xAA33,
        end: 0xAA35,
        value: "L",
    },
    PropertyRange {
        start: 0xAA35,
        end: 0xAA37,
        value: "NSM",
    },
    PropertyRange {
        start: 0xAA40,
        end: 0xAA43,
        value: "L",
    },
    PropertyRange {
        start: 0xAA43,
        end: 0xAA44,
        value: "NSM",
    },
    PropertyRange {
        start: 0xAA44,
        end: 0xAA4C,
        value: "L",
    },
    PropertyRange {
        start: 0xAA4C,
        end: 0xAA4D,
        value: "NSM",
    },
    PropertyRange {
        start: 0xAA4D,
        end: 0xAA4E,
        value: "L",
    },
    PropertyRange {
        start: 0xAA50,
        end: 0xAA5A,
        value: "L",
    },
    PropertyRange {
        start: 0xAA5C,
        end: 0xAA7C,
        value: "L",
    },
    PropertyRange {
        start: 0xAA7C,
        end: 0xAA7D,
        value: "NSM",
    },
    PropertyRange {
        start: 0xAA7D,
        end: 0xAAB0,
        value: "L",
    },
    PropertyRange {
        start: 0xAAB0,
        end: 0xAAB1,
        value: "NSM",
    },
    PropertyRange {
        start: 0xAAB1,
        end: 0xAAB2,
        value: "L",
    },
    PropertyRange {
        start: 0xAAB2,
        end: 0xAAB5,
        value: "NSM",
    },
    PropertyRange {
        start: 0xAAB5,
        end: 0xAAB7,
        value: "L",
    },
    PropertyRange {
        start: 0xAAB7,
        end: 0xAAB9,
        value: "NSM",
    },
    PropertyRange {
        start: 0xAAB9,
        end: 0xAABE,
        value: "L",
    },
    PropertyRange {
        start: 0xAABE,
        end: 0xAAC0,
        value: "NSM",
    },
    PropertyRange {
        start: 0xAAC0,
        end: 0xAAC1,
        value: "L",
    },
    PropertyRange {
        start: 0xAAC1,
        end: 0xAAC2,
        value: "NSM",
    },
    PropertyRange {
        start: 0xAAC2,
        end: 0xAAC3,
        value: "L",
    },
    PropertyRange {
        start: 0xAADB,
        end: 0xAAEC,
        value: "L",
    },
    PropertyRange {
        start: 0xAAEC,
        end: 0xAAEE,
        value: "NSM",
    },
    PropertyRange {
        start: 0xAAEE,
        end: 0xAAF6,
        value: "L",
    },
    PropertyRange {
        start: 0xAAF6,
        end: 0xAAF7,
        value: "NSM",
    },
    PropertyRange {
        start: 0xAB01,
        end: 0xAB07,
        value: "L",
    },
    PropertyRange {
        start: 0xAB09,
        end: 0xAB0F,
        value: "L",
    },
    PropertyRange {
        start: 0xAB11,
        end: 0xAB17,
        value: "L",
    },
    PropertyRange {
        start: 0xAB20,
        end: 0xAB27,
        value: "L",
    },
    PropertyRange {
        start: 0xAB28,
        end: 0xAB2F,
        value: "L",
    },
    PropertyRange {
        start: 0xAB30,
        end: 0xAB6A,
        value: "L",
    },
    PropertyRange {
        start: 0xAB6A,
        end: 0xAB6C,
        value: "ON",
    },
    PropertyRange {
        start: 0xAB70,
        end: 0xABE5,
        value: "L",
    },
    PropertyRange {
        start: 0xABE5,
        end: 0xABE6,
        value: "NSM",
    },
    PropertyRange {
        start: 0xABE6,
        end: 0xABE8,
        value: "L",
    },
    PropertyRange {
        start: 0xABE8,
        end: 0xABE9,
        value: "NSM",
    },
    PropertyRange {
        start: 0xABE9,
        end: 0xABED,
        value: "L",
    },
    PropertyRange {
        start: 0xABED,
        end: 0xABEE,
        value: "NSM",
    },
    PropertyRange {
        start: 0xABF0,
        end: 0xABFA,
        value: "L",
    },
    PropertyRange {
        start: 0xAC00,
        end: 0xD7A4,
        value: "L",
    },
    PropertyRange {
        start: 0xD7B0,
        end: 0xD7C7,
        value: "L",
    },
    PropertyRange {
        start: 0xD7CB,
        end: 0xD7FC,
        value: "L",
    },
    PropertyRange {
        start: 0xD800,
        end: 0xFA6E,
        value: "L",
    },
    PropertyRange {
        start: 0xFA70,
        end: 0xFADA,
        value: "L",
    },
    PropertyRange {
        start: 0xFB00,
        end: 0xFB07,
        value: "L",
    },
    PropertyRange {
        start: 0xFB13,
        end: 0xFB18,
        value: "L",
    },
    PropertyRange {
        start: 0xFB1D,
        end: 0xFB1E,
        value: "R",
    },
    PropertyRange {
        start: 0xFB1E,
        end: 0xFB1F,
        value: "NSM",
    },
    PropertyRange {
        start: 0xFB1F,
        end: 0xFB29,
        value: "R",
    },
    PropertyRange {
        start: 0xFB29,
        end: 0xFB2A,
        value: "ES",
    },
    PropertyRange {
        start: 0xFB2A,
        end: 0xFB37,
        value: "R",
    },
    PropertyRange {
        start: 0xFB38,
        end: 0xFB3D,
        value: "R",
    },
    PropertyRange {
        start: 0xFB3E,
        end: 0xFB3F,
        value: "R",
    },
    PropertyRange {
        start: 0xFB40,
        end: 0xFB42,
        value: "R",
    },
    PropertyRange {
        start: 0xFB43,
        end: 0xFB45,
        value: "R",
    },
    PropertyRange {
        start: 0xFB46,
        end: 0xFB50,
        value: "R",
    },
    PropertyRange {
        start: 0xFB50,
        end: 0xFBC3,
        value: "AL",
    },
    PropertyRange {
        start: 0xFBD3,
        end: 0xFD3E,
        value: "AL",
    },
    PropertyRange {
        start: 0xFD3E,
        end: 0xFD50,
        value: "ON",
    },
    PropertyRange {
        start: 0xFD50,
        end: 0xFD90,
        value: "AL",
    },
    PropertyRange {
        start: 0xFD92,
        end: 0xFDC8,
        value: "AL",
    },
    PropertyRange {
        start: 0xFDCF,
        end: 0xFDD0,
        value: "ON",
    },
    PropertyRange {
        start: 0xFDF0,
        end: 0xFDFD,
        value: "AL",
    },
    PropertyRange {
        start: 0xFDFD,
        end: 0xFE00,
        value: "ON",
    },
    PropertyRange {
        start: 0xFE00,
        end: 0xFE10,
        value: "NSM",
    },
    PropertyRange {
        start: 0xFE10,
        end: 0xFE1A,
        value: "ON",
    },
    PropertyRange {
        start: 0xFE20,
        end: 0xFE30,
        value: "NSM",
    },
    PropertyRange {
        start: 0xFE30,
        end: 0xFE50,
        value: "ON",
    },
    PropertyRange {
        start: 0xFE50,
        end: 0xFE51,
        value: "CS",
    },
    PropertyRange {
        start: 0xFE51,
        end: 0xFE52,
        value: "ON",
    },
    PropertyRange {
        start: 0xFE52,
        end: 0xFE53,
        value: "CS",
    },
    PropertyRange {
        start: 0xFE54,
        end: 0xFE55,
        value: "ON",
    },
    PropertyRange {
        start: 0xFE55,
        end: 0xFE56,
        value: "CS",
    },
    PropertyRange {
        start: 0xFE56,
        end: 0xFE5F,
        value: "ON",
    },
    PropertyRange {
        start: 0xFE5F,
        end: 0xFE60,
        value: "ET",
    },
    PropertyRange {
        start: 0xFE60,
        end: 0xFE62,
        value: "ON",
    },
    PropertyRange {
        start: 0xFE62,
        end: 0xFE64,
        value: "ES",
    },
    PropertyRange {
        start: 0xFE64,
        end: 0xFE67,
        value: "ON",
    },
    PropertyRange {
        start: 0xFE68,
        end: 0xFE69,
        value: "ON",
    },
    PropertyRange {
        start: 0xFE69,
        end: 0xFE6B,
        value: "ET",
    },
    PropertyRange {
        start: 0xFE6B,
        end: 0xFE6C,
        value: "ON",
    },
    PropertyRange {
        start: 0xFE70,
        end: 0xFE75,
        value: "AL",
    },
    PropertyRange {
        start: 0xFE76,
        end: 0xFEFD,
        value: "AL",
    },
    PropertyRange {
        start: 0xFEFF,
        end: 0xFF00,
        value: "BN",
    },
    PropertyRange {
        start: 0xFF01,
        end: 0xFF03,
        value: "ON",
    },
    PropertyRange {
        start: 0xFF03,
        end: 0xFF06,
        value: "ET",
    },
    PropertyRange {
        start: 0xFF06,
        end: 0xFF0B,
        value: "ON",
    },
    PropertyRange {
        start: 0xFF0B,
        end: 0xFF0C,
        value: "ES",
    },
    PropertyRange {
        start: 0xFF0C,
        end: 0xFF0D,
        value: "CS",
    },
    PropertyRange {
        start: 0xFF0D,
        end: 0xFF0E,
        value: "ES",
    },
    PropertyRange {
        start: 0xFF0E,
        end: 0xFF10,
        value: "CS",
    },
    PropertyRange {
        start: 0xFF10,
        end: 0xFF1A,
        value: "EN",
    },
    PropertyRange {
        start: 0xFF1A,
        end: 0xFF1B,
        value: "CS",
    },
    PropertyRange {
        start: 0xFF1B,
        end: 0xFF21,
        value: "ON",
    },
    PropertyRange {
        start: 0xFF21,
        end: 0xFF3B,
        value: "L",
    },
    PropertyRange {
        start: 0xFF3B,
        end: 0xFF41,
        value: "ON",
    },
    PropertyRange {
        start: 0xFF41,
        end: 0xFF5B,
        value: "L",
    },
    PropertyRange {
        start: 0xFF5B,
        end: 0xFF66,
        value: "ON",
    },
    PropertyRange {
        start: 0xFF66,
        end: 0xFFBF,
        value: "L",
    },
    PropertyRange {
        start: 0xFFC2,
        end: 0xFFC8,
        value: "L",
    },
    PropertyRange {
        start: 0xFFCA,
        end: 0xFFD0,
        value: "L",
    },
    PropertyRange {
        start: 0xFFD2,
        end: 0xFFD8,
        value: "L",
    },
    PropertyRange {
        start: 0xFFDA,
        end: 0xFFDD,
        value: "L",
    },
    PropertyRange {
        start: 0xFFE0,
        end: 0xFFE2,
        value: "ET",
    },
    PropertyRange {
        start: 0xFFE2,
        end: 0xFFE5,
        value: "ON",
    },
    PropertyRange {
        start: 0xFFE5,
        end: 0xFFE7,
        value: "ET",
    },
    PropertyRange {
        start: 0xFFE8,
        end: 0xFFEF,
        value: "ON",
    },
    PropertyRange {
        start: 0xFFF9,
        end: 0xFFFE,
        value: "ON",
    },
    PropertyRange {
        start: 0x10000,
        end: 0x1000C,
        value: "L",
    },
    PropertyRange {
        start: 0x1000D,
        end: 0x10027,
        value: "L",
    },
    PropertyRange {
        start: 0x10028,
        end: 0x1003B,
        value: "L",
    },
    PropertyRange {
        start: 0x1003C,
        end: 0x1003E,
        value: "L",
    },
    PropertyRange {
        start: 0x1003F,
        end: 0x1004E,
        value: "L",
    },
    PropertyRange {
        start: 0x10050,
        end: 0x1005E,
        value: "L",
    },
    PropertyRange {
        start: 0x10080,
        end: 0x100FB,
        value: "L",
    },
    PropertyRange {
        start: 0x10100,
        end: 0x10101,
        value: "L",
    },
    PropertyRange {
        start: 0x10101,
        end: 0x10102,
        value: "ON",
    },
    PropertyRange {
        start: 0x10102,
        end: 0x10103,
        value: "L",
    },
    PropertyRange {
        start: 0x10107,
        end: 0x10134,
        value: "L",
    },
    PropertyRange {
        start: 0x10137,
        end: 0x10140,
        value: "L",
    },
    PropertyRange {
        start: 0x10140,
        end: 0x1018D,
        value: "ON",
    },
    PropertyRange {
        start: 0x1018D,
        end: 0x1018F,
        value: "L",
    },
    PropertyRange {
        start: 0x10190,
        end: 0x1019D,
        value: "ON",
    },
    PropertyRange {
        start: 0x101A0,
        end: 0x101A1,
        value: "ON",
    },
    PropertyRange {
        start: 0x101D0,
        end: 0x101FD,
        value: "L",
    },
    PropertyRange {
        start: 0x101FD,
        end: 0x101FE,
        value: "NSM",
    },
    PropertyRange {
        start: 0x10280,
        end: 0x1029D,
        value: "L",
    },
    PropertyRange {
        start: 0x102A0,
        end: 0x102D1,
        value: "L",
    },
    PropertyRange {
        start: 0x102E0,
        end: 0x102E1,
        value: "NSM",
    },
    PropertyRange {
        start: 0x102E1,
        end: 0x102FC,
        value: "EN",
    },
    PropertyRange {
        start: 0x10300,
        end: 0x10324,
        value: "L",
    },
    PropertyRange {
        start: 0x1032D,
        end: 0x1034B,
        value: "L",
    },
    PropertyRange {
        start: 0x10350,
        end: 0x10376,
        value: "L",
    },
    PropertyRange {
        start: 0x10376,
        end: 0x1037B,
        value: "NSM",
    },
    PropertyRange {
        start: 0x10380,
        end: 0x1039E,
        value: "L",
    },
    PropertyRange {
        start: 0x1039F,
        end: 0x103C4,
        value: "L",
    },
    PropertyRange {
        start: 0x103C8,
        end: 0x103D6,
        value: "L",
    },
    PropertyRange {
        start: 0x10400,
        end: 0x1049E,
        value: "L",
    },
    PropertyRange {
        start: 0x104A0,
        end: 0x104AA,
        value: "L",
    },
    PropertyRange {
        start: 0x104B0,
        end: 0x104D4,
        value: "L",
    },
    PropertyRange {
        start: 0x104D8,
        end: 0x104FC,
        value: "L",
    },
    PropertyRange {
        start: 0x10500,
        end: 0x10528,
        value: "L",
    },
    PropertyRange {
        start: 0x10530,
        end: 0x10564,
        value: "L",
    },
    PropertyRange {
        start: 0x1056F,
        end: 0x1057B,
        value: "L",
    },
    PropertyRange {
        start: 0x1057C,
        end: 0x1058B,
        value: "L",
    },
    PropertyRange {
        start: 0x1058C,
        end: 0x10593,
        value: "L",
    },
    PropertyRange {
        start: 0x10594,
        end: 0x10596,
        value: "L",
    },
    PropertyRange {
        start: 0x10597,
        end: 0x105A2,
        value: "L",
    },
    PropertyRange {
        start: 0x105A3,
        end: 0x105B2,
        value: "L",
    },
    PropertyRange {
        start: 0x105B3,
        end: 0x105BA,
        value: "L",
    },
    PropertyRange {
        start: 0x105BB,
        end: 0x105BD,
        value: "L",
    },
    PropertyRange {
        start: 0x105C0,
        end: 0x105F4,
        value: "L",
    },
    PropertyRange {
        start: 0x10600,
        end: 0x10737,
        value: "L",
    },
    PropertyRange {
        start: 0x10740,
        end: 0x10756,
        value: "L",
    },
    PropertyRange {
        start: 0x10760,
        end: 0x10768,
        value: "L",
    },
    PropertyRange {
        start: 0x10780,
        end: 0x10786,
        value: "L",
    },
    PropertyRange {
        start: 0x10787,
        end: 0x107B1,
        value: "L",
    },
    PropertyRange {
        start: 0x107B2,
        end: 0x107BB,
        value: "L",
    },
    PropertyRange {
        start: 0x10800,
        end: 0x10806,
        value: "R",
    },
    PropertyRange {
        start: 0x10808,
        end: 0x10809,
        value: "R",
    },
    PropertyRange {
        start: 0x1080A,
        end: 0x10836,
        value: "R",
    },
    PropertyRange {
        start: 0x10837,
        end: 0x10839,
        value: "R",
    },
    PropertyRange {
        start: 0x1083C,
        end: 0x1083D,
        value: "R",
    },
    PropertyRange {
        start: 0x1083F,
        end: 0x10856,
        value: "R",
    },
    PropertyRange {
        start: 0x10857,
        end: 0x1089F,
        value: "R",
    },
    PropertyRange {
        start: 0x108A7,
        end: 0x108B0,
        value: "R",
    },
    PropertyRange {
        start: 0x108E0,
        end: 0x108F3,
        value: "R",
    },
    PropertyRange {
        start: 0x108F4,
        end: 0x108F6,
        value: "R",
    },
    PropertyRange {
        start: 0x108FB,
        end: 0x1091C,
        value: "R",
    },
    PropertyRange {
        start: 0x1091F,
        end: 0x10920,
        value: "ON",
    },
    PropertyRange {
        start: 0x10920,
        end: 0x1093A,
        value: "R",
    },
    PropertyRange {
        start: 0x1093F,
        end: 0x10940,
        value: "R",
    },
    PropertyRange {
        start: 0x10980,
        end: 0x109B8,
        value: "R",
    },
    PropertyRange {
        start: 0x109BC,
        end: 0x109D0,
        value: "R",
    },
    PropertyRange {
        start: 0x109D2,
        end: 0x10A01,
        value: "R",
    },
    PropertyRange {
        start: 0x10A01,
        end: 0x10A04,
        value: "NSM",
    },
    PropertyRange {
        start: 0x10A05,
        end: 0x10A07,
        value: "NSM",
    },
    PropertyRange {
        start: 0x10A0C,
        end: 0x10A10,
        value: "NSM",
    },
    PropertyRange {
        start: 0x10A10,
        end: 0x10A14,
        value: "R",
    },
    PropertyRange {
        start: 0x10A15,
        end: 0x10A18,
        value: "R",
    },
    PropertyRange {
        start: 0x10A19,
        end: 0x10A36,
        value: "R",
    },
    PropertyRange {
        start: 0x10A38,
        end: 0x10A3B,
        value: "NSM",
    },
    PropertyRange {
        start: 0x10A3F,
        end: 0x10A40,
        value: "NSM",
    },
    PropertyRange {
        start: 0x10A40,
        end: 0x10A49,
        value: "R",
    },
    PropertyRange {
        start: 0x10A50,
        end: 0x10A59,
        value: "R",
    },
    PropertyRange {
        start: 0x10A60,
        end: 0x10AA0,
        value: "R",
    },
    PropertyRange {
        start: 0x10AC0,
        end: 0x10AE5,
        value: "R",
    },
    PropertyRange {
        start: 0x10AE5,
        end: 0x10AE7,
        value: "NSM",
    },
    PropertyRange {
        start: 0x10AEB,
        end: 0x10AF7,
        value: "R",
    },
    PropertyRange {
        start: 0x10B00,
        end: 0x10B36,
        value: "R",
    },
    PropertyRange {
        start: 0x10B39,
        end: 0x10B40,
        value: "ON",
    },
    PropertyRange {
        start: 0x10B40,
        end: 0x10B56,
        value: "R",
    },
    PropertyRange {
        start: 0x10B58,
        end: 0x10B73,
        value: "R",
    },
    PropertyRange {
        start: 0x10B78,
        end: 0x10B92,
        value: "R",
    },
    PropertyRange {
        start: 0x10B99,
        end: 0x10B9D,
        value: "R",
    },
    PropertyRange {
        start: 0x10BA9,
        end: 0x10BB0,
        value: "R",
    },
    PropertyRange {
        start: 0x10C00,
        end: 0x10C49,
        value: "R",
    },
    PropertyRange {
        start: 0x10C80,
        end: 0x10CB3,
        value: "R",
    },
    PropertyRange {
        start: 0x10CC0,
        end: 0x10CF3,
        value: "R",
    },
    PropertyRange {
        start: 0x10CFA,
        end: 0x10D00,
        value: "R",
    },
    PropertyRange {
        start: 0x10D00,
        end: 0x10D24,
        value: "AL",
    },
    PropertyRange {
        start: 0x10D24,
        end: 0x10D28,
        value: "NSM",
    },
    PropertyRange {
        start: 0x10D30,
        end: 0x10D3A,
        value: "AN",
    },
    PropertyRange {
        start: 0x10D40,
        end: 0x10D4A,
        value: "AN",
    },
    PropertyRange {
        start: 0x10D4A,
        end: 0x10D66,
        value: "R",
    },
    PropertyRange {
        start: 0x10D69,
        end: 0x10D6E,
        value: "NSM",
    },
    PropertyRange {
        start: 0x10D6E,
        end: 0x10D6F,
        value: "ON",
    },
    PropertyRange {
        start: 0x10D6F,
        end: 0x10D86,
        value: "R",
    },
    PropertyRange {
        start: 0x10D8E,
        end: 0x10D90,
        value: "R",
    },
    PropertyRange {
        start: 0x10E60,
        end: 0x10E7F,
        value: "AN",
    },
    PropertyRange {
        start: 0x10E80,
        end: 0x10EAA,
        value: "R",
    },
    PropertyRange {
        start: 0x10EAB,
        end: 0x10EAD,
        value: "NSM",
    },
    PropertyRange {
        start: 0x10EAD,
        end: 0x10EAE,
        value: "R",
    },
    PropertyRange {
        start: 0x10EB0,
        end: 0x10EB2,
        value: "R",
    },
    PropertyRange {
        start: 0x10EC2,
        end: 0x10EC5,
        value: "AL",
    },
    PropertyRange {
        start: 0x10EFC,
        end: 0x10F00,
        value: "NSM",
    },
    PropertyRange {
        start: 0x10F00,
        end: 0x10F28,
        value: "R",
    },
    PropertyRange {
        start: 0x10F30,
        end: 0x10F46,
        value: "AL",
    },
    PropertyRange {
        start: 0x10F46,
        end: 0x10F51,
        value: "NSM",
    },
    PropertyRange {
        start: 0x10F51,
        end: 0x10F5A,
        value: "AL",
    },
    PropertyRange {
        start: 0x10F70,
        end: 0x10F82,
        value: "R",
    },
    PropertyRange {
        start: 0x10F82,
        end: 0x10F86,
        value: "NSM",
    },
    PropertyRange {
        start: 0x10F86,
        end: 0x10F8A,
        value: "R",
    },
    PropertyRange {
        start: 0x10FB0,
        end: 0x10FCC,
        value: "R",
    },
    PropertyRange {
        start: 0x10FE0,
        end: 0x10FF7,
        value: "R",
    },
    PropertyRange {
        start: 0x11000,
        end: 0x11001,
        value: "L",
    },
    PropertyRange {
        start: 0x11001,
        end: 0x11002,
        value: "NSM",
    },
    PropertyRange {
        start: 0x11002,
        end: 0x11038,
        value: "L",
    },
    PropertyRange {
        start: 0x11038,
        end: 0x11047,
        value: "NSM",
    },
    PropertyRange {
        start: 0x11047,
        end: 0x1104E,
        value: "L",
    },
    PropertyRange {
        start: 0x11052,
        end: 0x11066,
        value: "ON",
    },
    PropertyRange {
        start: 0x11066,
        end: 0x11070,
        value: "L",
    },
    PropertyRange {
        start: 0x11070,
        end: 0x11071,
        value: "NSM",
    },
    PropertyRange {
        start: 0x11071,
        end: 0x11073,
        value: "L",
    },
    PropertyRange {
        start: 0x11073,
        end: 0x11075,
        value: "NSM",
    },
    PropertyRange {
        start: 0x11075,
        end: 0x11076,
        value: "L",
    },
    PropertyRange {
        start: 0x1107F,
        end: 0x11082,
        value: "NSM",
    },
    PropertyRange {
        start: 0x11082,
        end: 0x110B3,
        value: "L",
    },
    PropertyRange {
        start: 0x110B3,
        end: 0x110B7,
        value: "NSM",
    },
    PropertyRange {
        start: 0x110B7,
        end: 0x110B9,
        value: "L",
    },
    PropertyRange {
        start: 0x110B9,
        end: 0x110BB,
        value: "NSM",
    },
    PropertyRange {
        start: 0x110BB,
        end: 0x110C2,
        value: "L",
    },
    PropertyRange {
        start: 0x110C2,
        end: 0x110C3,
        value: "NSM",
    },
    PropertyRange {
        start: 0x110CD,
        end: 0x110CE,
        value: "L",
    },
    PropertyRange {
        start: 0x110D0,
        end: 0x110E9,
        value: "L",
    },
    PropertyRange {
        start: 0x110F0,
        end: 0x110FA,
        value: "L",
    },
    PropertyRange {
        start: 0x11100,
        end: 0x11103,
        value: "NSM",
    },
    PropertyRange {
        start: 0x11103,
        end: 0x11127,
        value: "L",
    },
    PropertyRange {
        start: 0x11127,
        end: 0x1112C,
        value: "NSM",
    },
    PropertyRange {
        start: 0x1112C,
        end: 0x1112D,
        value: "L",
    },
    PropertyRange {
        start: 0x1112D,
        end: 0x11135,
        value: "NSM",
    },
    PropertyRange {
        start: 0x11136,
        end: 0x11148,
        value: "L",
    },
    PropertyRange {
        start: 0x11150,
        end: 0x11173,
        value: "L",
    },
    PropertyRange {
        start: 0x11173,
        end: 0x11174,
        value: "NSM",
    },
    PropertyRange {
        start: 0x11174,
        end: 0x11177,
        value: "L",
    },
    PropertyRange {
        start: 0x11180,
        end: 0x11182,
        value: "NSM",
    },
    PropertyRange {
        start: 0x11182,
        end: 0x111B6,
        value: "L",
    },
    PropertyRange {
        start: 0x111B6,
        end: 0x111BF,
        value: "NSM",
    },
    PropertyRange {
        start: 0x111BF,
        end: 0x111C9,
        value: "L",
    },
    PropertyRange {
        start: 0x111C9,
        end: 0x111CD,
        value: "NSM",
    },
    PropertyRange {
        start: 0x111CD,
        end: 0x111CF,
        value: "L",
    },
    PropertyRange {
        start: 0x111CF,
        end: 0x111D0,
        value: "NSM",
    },
    PropertyRange {
        start: 0x111D0,
        end: 0x111E0,
        value: "L",
    },
    PropertyRange {
        start: 0x111E1,
        end: 0x111F5,
        value: "L",
    },
    PropertyRange {
        start: 0x11200,
        end: 0x11212,
        value: "L",
    },
    PropertyRange {
        start: 0x11213,
        end: 0x1122F,
        value: "L",
    },
    PropertyRange {
        start: 0x1122F,
        end: 0x11232,
        value: "NSM",
    },
    PropertyRange {
        start: 0x11232,
        end: 0x11234,
        value: "L",
    },
    PropertyRange {
        start: 0x11234,
        end: 0x11235,
        value: "NSM",
    },
    PropertyRange {
        start: 0x11235,
        end: 0x11236,
        value: "L",
    },
    PropertyRange {
        start: 0x11236,
        end: 0x11238,
        value: "NSM",
    },
    PropertyRange {
        start: 0x11238,
        end: 0x1123E,
        value: "L",
    },
    PropertyRange {
        start: 0x1123E,
        end: 0x1123F,
        value: "NSM",
    },
    PropertyRange {
        start: 0x1123F,
        end: 0x11241,
        value: "L",
    },
    PropertyRange {
        start: 0x11241,
        end: 0x11242,
        value: "NSM",
    },
    PropertyRange {
        start: 0x11280,
        end: 0x11287,
        value: "L",
    },
    PropertyRange {
        start: 0x11288,
        end: 0x11289,
        value: "L",
    },
    PropertyRange {
        start: 0x1128A,
        end: 0x1128E,
        value: "L",
    },
    PropertyRange {
        start: 0x1128F,
        end: 0x1129E,
        value: "L",
    },
    PropertyRange {
        start: 0x1129F,
        end: 0x112AA,
        value: "L",
    },
    PropertyRange {
        start: 0x112B0,
        end: 0x112DF,
        value: "L",
    },
    PropertyRange {
        start: 0x112DF,
        end: 0x112E0,
        value: "NSM",
    },
    PropertyRange {
        start: 0x112E0,
        end: 0x112E3,
        value: "L",
    },
    PropertyRange {
        start: 0x112E3,
        end: 0x112EB,
        value: "NSM",
    },
    PropertyRange {
        start: 0x112F0,
        end: 0x112FA,
        value: "L",
    },
    PropertyRange {
        start: 0x11300,
        end: 0x11302,
        value: "NSM",
    },
    PropertyRange {
        start: 0x11302,
        end: 0x11304,
        value: "L",
    },
    PropertyRange {
        start: 0x11305,
        end: 0x1130D,
        value: "L",
    },
    PropertyRange {
        start: 0x1130F,
        end: 0x11311,
        value: "L",
    },
    PropertyRange {
        start: 0x11313,
        end: 0x11329,
        value: "L",
    },
    PropertyRange {
        start: 0x1132A,
        end: 0x11331,
        value: "L",
    },
    PropertyRange {
        start: 0x11332,
        end: 0x11334,
        value: "L",
    },
    PropertyRange {
        start: 0x11335,
        end: 0x1133A,
        value: "L",
    },
    PropertyRange {
        start: 0x1133B,
        end: 0x1133D,
        value: "NSM",
    },
    PropertyRange {
        start: 0x1133D,
        end: 0x11340,
        value: "L",
    },
    PropertyRange {
        start: 0x11340,
        end: 0x11341,
        value: "NSM",
    },
    PropertyRange {
        start: 0x11341,
        end: 0x11345,
        value: "L",
    },
    PropertyRange {
        start: 0x11347,
        end: 0x11349,
        value: "L",
    },
    PropertyRange {
        start: 0x1134B,
        end: 0x1134E,
        value: "L",
    },
    PropertyRange {
        start: 0x11350,
        end: 0x11351,
        value: "L",
    },
    PropertyRange {
        start: 0x11357,
        end: 0x11358,
        value: "L",
    },
    PropertyRange {
        start: 0x1135D,
        end: 0x11364,
        value: "L",
    },
    PropertyRange {
        start: 0x11366,
        end: 0x1136D,
        value: "NSM",
    },
    PropertyRange {
        start: 0x11370,
        end: 0x11375,
        value: "NSM",
    },
    PropertyRange {
        start: 0x11380,
        end: 0x1138A,
        value: "L",
    },
    PropertyRange {
        start: 0x1138B,
        end: 0x1138C,
        value: "L",
    },
    PropertyRange {
        start: 0x1138E,
        end: 0x1138F,
        value: "L",
    },
    PropertyRange {
        start: 0x11390,
        end: 0x113B6,
        value: "L",
    },
    PropertyRange {
        start: 0x113B7,
        end: 0x113BB,
        value: "L",
    },
    PropertyRange {
        start: 0x113BB,
        end: 0x113C1,
        value: "NSM",
    },
    PropertyRange {
        start: 0x113C2,
        end: 0x113C3,
        value: "L",
    },
    PropertyRange {
        start: 0x113C5,
        end: 0x113C6,
        value: "L",
    },
    PropertyRange {
        start: 0x113C7,
        end: 0x113CB,
        value: "L",
    },
    PropertyRange {
        start: 0x113CC,
        end: 0x113CE,
        value: "L",
    },
    PropertyRange {
        start: 0x113CE,
        end: 0x113CF,
        value: "NSM",
    },
    PropertyRange {
        start: 0x113CF,
        end: 0x113D0,
        value: "L",
    },
    PropertyRange {
        start: 0x113D0,
        end: 0x113D1,
        value: "NSM",
    },
    PropertyRange {
        start: 0x113D1,
        end: 0x113D2,
        value: "L",
    },
    PropertyRange {
        start: 0x113D2,
        end: 0x113D3,
        value: "NSM",
    },
    PropertyRange {
        start: 0x113D3,
        end: 0x113D6,
        value: "L",
    },
    PropertyRange {
        start: 0x113D7,
        end: 0x113D9,
        value: "L",
    },
    PropertyRange {
        start: 0x113E1,
        end: 0x113E3,
        value: "NSM",
    },
    PropertyRange {
        start: 0x11400,
        end: 0x11438,
        value: "L",
    },
    PropertyRange {
        start: 0x11438,
        end: 0x11440,
        value: "NSM",
    },
    PropertyRange {
        start: 0x11440,
        end: 0x11442,
        value: "L",
    },
    PropertyRange {
        start: 0x11442,
        end: 0x11445,
        value: "NSM",
    },
    PropertyRange {
        start: 0x11445,
        end: 0x11446,
        value: "L",
    },
    PropertyRange {
        start: 0x11446,
        end: 0x11447,
        value: "NSM",
    },
    PropertyRange {
        start: 0x11447,
        end: 0x1145C,
        value: "L",
    },
    PropertyRange {
        start: 0x1145D,
        end: 0x1145E,
        value: "L",
    },
    PropertyRange {
        start: 0x1145E,
        end: 0x1145F,
        value: "NSM",
    },
    PropertyRange {
        start: 0x1145F,
        end: 0x11462,
        value: "L",
    },
    PropertyRange {
        start: 0x11480,
        end: 0x114B3,
        value: "L",
    },
    PropertyRange {
        start: 0x114B3,
        end: 0x114B9,
        value: "NSM",
    },
    PropertyRange {
        start: 0x114B9,
        end: 0x114BA,
        value: "L",
    },
    PropertyRange {
        start: 0x114BA,
        end: 0x114BB,
        value: "NSM",
    },
    PropertyRange {
        start: 0x114BB,
        end: 0x114BF,
        value: "L",
    },
    PropertyRange {
        start: 0x114BF,
        end: 0x114C1,
        value: "NSM",
    },
    PropertyRange {
        start: 0x114C1,
        end: 0x114C2,
        value: "L",
    },
    PropertyRange {
        start: 0x114C2,
        end: 0x114C4,
        value: "NSM",
    },
    PropertyRange {
        start: 0x114C4,
        end: 0x114C8,
        value: "L",
    },
    PropertyRange {
        start: 0x114D0,
        end: 0x114DA,
        value: "L",
    },
    PropertyRange {
        start: 0x11580,
        end: 0x115B2,
        value: "L",
    },
    PropertyRange {
        start: 0x115B2,
        end: 0x115B6,
        value: "NSM",
    },
    PropertyRange {
        start: 0x115B8,
        end: 0x115BC,
        value: "L",
    },
    PropertyRange {
        start: 0x115BC,
        end: 0x115BE,
        value: "NSM",
    },
    PropertyRange {
        start: 0x115BE,
        end: 0x115BF,
        value: "L",
    },
    PropertyRange {
        start: 0x115BF,
        end: 0x115C1,
        value: "NSM",
    },
    PropertyRange {
        start: 0x115C1,
        end: 0x115DC,
        value: "L",
    },
    PropertyRange {
        start: 0x115DC,
        end: 0x115DE,
        value: "NSM",
    },
    PropertyRange {
        start: 0x11600,
        end: 0x11633,
        value: "L",
    },
    PropertyRange {
        start: 0x11633,
        end: 0x1163B,
        value: "NSM",
    },
    PropertyRange {
        start: 0x1163B,
        end: 0x1163D,
        value: "L",
    },
    PropertyRange {
        start: 0x1163D,
        end: 0x1163E,
        value: "NSM",
    },
    PropertyRange {
        start: 0x1163E,
        end: 0x1163F,
        value: "L",
    },
    PropertyRange {
        start: 0x1163F,
        end: 0x11641,
        value: "NSM",
    },
    PropertyRange {
        start: 0x11641,
        end: 0x11645,
        value: "L",
    },
    PropertyRange {
        start: 0x11650,
        end: 0x1165A,
        value: "L",
    },
    PropertyRange {
        start: 0x11660,
        end: 0x1166D,
        value: "ON",
    },
    PropertyRange {
        start: 0x11680,
        end: 0x116AB,
        value: "L",
    },
    PropertyRange {
        start: 0x116AB,
        end: 0x116AC,
        value: "NSM",
    },
    PropertyRange {
        start: 0x116AC,
        end: 0x116AD,
        value: "L",
    },
    PropertyRange {
        start: 0x116AD,
        end: 0x116AE,
        value: "NSM",
    },
    PropertyRange {
        start: 0x116AE,
        end: 0x116B0,
        value: "L",
    },
    PropertyRange {
        start: 0x116B0,
        end: 0x116B6,
        value: "NSM",
    },
    PropertyRange {
        start: 0x116B6,
        end: 0x116B7,
        value: "L",
    },
    PropertyRange {
        start: 0x116B7,
        end: 0x116B8,
        value: "NSM",
    },
    PropertyRange {
        start: 0x116B8,
        end: 0x116BA,
        value: "L",
    },
    PropertyRange {
        start: 0x116C0,
        end: 0x116CA,
        value: "L",
    },
    PropertyRange {
        start: 0x116D0,
        end: 0x116E4,
        value: "L",
    },
    PropertyRange {
        start: 0x11700,
        end: 0x1171B,
        value: "L",
    },
    PropertyRange {
        start: 0x1171D,
        end: 0x1171E,
        value: "NSM",
    },
    PropertyRange {
        start: 0x1171E,
        end: 0x1171F,
        value: "L",
    },
    PropertyRange {
        start: 0x1171F,
        end: 0x11720,
        value: "NSM",
    },
    PropertyRange {
        start: 0x11720,
        end: 0x11722,
        value: "L",
    },
    PropertyRange {
        start: 0x11722,
        end: 0x11726,
        value: "NSM",
    },
    PropertyRange {
        start: 0x11726,
        end: 0x11727,
        value: "L",
    },
    PropertyRange {
        start: 0x11727,
        end: 0x1172C,
        value: "NSM",
    },
    PropertyRange {
        start: 0x11730,
        end: 0x11747,
        value: "L",
    },
    PropertyRange {
        start: 0x11800,
        end: 0x1182F,
        value: "L",
    },
    PropertyRange {
        start: 0x1182F,
        end: 0x11838,
        value: "NSM",
    },
    PropertyRange {
        start: 0x11838,
        end: 0x11839,
        value: "L",
    },
    PropertyRange {
        start: 0x11839,
        end: 0x1183B,
        value: "NSM",
    },
    PropertyRange {
        start: 0x1183B,
        end: 0x1183C,
        value: "L",
    },
    PropertyRange {
        start: 0x118A0,
        end: 0x118F3,
        value: "L",
    },
    PropertyRange {
        start: 0x118FF,
        end: 0x11907,
        value: "L",
    },
    PropertyRange {
        start: 0x11909,
        end: 0x1190A,
        value: "L",
    },
    PropertyRange {
        start: 0x1190C,
        end: 0x11914,
        value: "L",
    },
    PropertyRange {
        start: 0x11915,
        end: 0x11917,
        value: "L",
    },
    PropertyRange {
        start: 0x11918,
        end: 0x11936,
        value: "L",
    },
    PropertyRange {
        start: 0x11937,
        end: 0x11939,
        value: "L",
    },
    PropertyRange {
        start: 0x1193B,
        end: 0x1193D,
        value: "NSM",
    },
    PropertyRange {
        start: 0x1193D,
        end: 0x1193E,
        value: "L",
    },
    PropertyRange {
        start: 0x1193E,
        end: 0x1193F,
        value: "NSM",
    },
    PropertyRange {
        start: 0x1193F,
        end: 0x11943,
        value: "L",
    },
    PropertyRange {
        start: 0x11943,
        end: 0x11944,
        value: "NSM",
    },
    PropertyRange {
        start: 0x11944,
        end: 0x11947,
        value: "L",
    },
    PropertyRange {
        start: 0x11950,
        end: 0x1195A,
        value: "L",
    },
    PropertyRange {
        start: 0x119A0,
        end: 0x119A8,
        value: "L",
    },
    PropertyRange {
        start: 0x119AA,
        end: 0x119D4,
        value: "L",
    },
    PropertyRange {
        start: 0x119D4,
        end: 0x119D8,
        value: "NSM",
    },
    PropertyRange {
        start: 0x119DA,
        end: 0x119DC,
        value: "NSM",
    },
    PropertyRange {
        start: 0x119DC,
        end: 0x119E0,
        value: "L",
    },
    PropertyRange {
        start: 0x119E0,
        end: 0x119E1,
        value: "NSM",
    },
    PropertyRange {
        start: 0x119E1,
        end: 0x119E5,
        value: "L",
    },
    PropertyRange {
        start: 0x11A00,
        end: 0x11A01,
        value: "L",
    },
    PropertyRange {
        start: 0x11A01,
        end: 0x11A07,
        value: "NSM",
    },
    PropertyRange {
        start: 0x11A07,
        end: 0x11A09,
        value: "L",
    },
    PropertyRange {
        start: 0x11A09,
        end: 0x11A0B,
        value: "NSM",
    },
    PropertyRange {
        start: 0x11A0B,
        end: 0x11A33,
        value: "L",
    },
    PropertyRange {
        start: 0x11A33,
        end: 0x11A39,
        value: "NSM",
    },
    PropertyRange {
        start: 0x11A39,
        end: 0x11A3B,
        value: "L",
    },
    PropertyRange {
        start: 0x11A3B,
        end: 0x11A3F,
        value: "NSM",
    },
    PropertyRange {
        start: 0x11A3F,
        end: 0x11A47,
        value: "L",
    },
    PropertyRange {
        start: 0x11A47,
        end: 0x11A48,
        value: "NSM",
    },
    PropertyRange {
        start: 0x11A50,
        end: 0x11A51,
        value: "L",
    },
    PropertyRange {
        start: 0x11A51,
        end: 0x11A57,
        value: "NSM",
    },
    PropertyRange {
        start: 0x11A57,
        end: 0x11A59,
        value: "L",
    },
    PropertyRange {
        start: 0x11A59,
        end: 0x11A5C,
        value: "NSM",
    },
    PropertyRange {
        start: 0x11A5C,
        end: 0x11A8A,
        value: "L",
    },
    PropertyRange {
        start: 0x11A8A,
        end: 0x11A97,
        value: "NSM",
    },
    PropertyRange {
        start: 0x11A97,
        end: 0x11A98,
        value: "L",
    },
    PropertyRange {
        start: 0x11A98,
        end: 0x11A9A,
        value: "NSM",
    },
    PropertyRange {
        start: 0x11A9A,
        end: 0x11AA3,
        value: "L",
    },
    PropertyRange {
        start: 0x11AB0,
        end: 0x11AF9,
        value: "L",
    },
    PropertyRange {
        start: 0x11B00,
        end: 0x11B0A,
        value: "L",
    },
    PropertyRange {
        start: 0x11BC0,
        end: 0x11BE2,
        value: "L",
    },
    PropertyRange {
        start: 0x11BF0,
        end: 0x11BFA,
        value: "L",
    },
    PropertyRange {
        start: 0x11C00,
        end: 0x11C09,
        value: "L",
    },
    PropertyRange {
        start: 0x11C0A,
        end: 0x11C30,
        value: "L",
    },
    PropertyRange {
        start: 0x11C30,
        end: 0x11C37,
        value: "NSM",
    },
    PropertyRange {
        start: 0x11C38,
        end: 0x11C3E,
        value: "NSM",
    },
    PropertyRange {
        start: 0x11C3E,
        end: 0x11C46,
        value: "L",
    },
    PropertyRange {
        start: 0x11C50,
        end: 0x11C6D,
        value: "L",
    },
    PropertyRange {
        start: 0x11C70,
        end: 0x11C90,
        value: "L",
    },
    PropertyRange {
        start: 0x11C92,
        end: 0x11CA8,
        value: "NSM",
    },
    PropertyRange {
        start: 0x11CA9,
        end: 0x11CAA,
        value: "L",
    },
    PropertyRange {
        start: 0x11CAA,
        end: 0x11CB1,
        value: "NSM",
    },
    PropertyRange {
        start: 0x11CB1,
        end: 0x11CB2,
        value: "L",
    },
    PropertyRange {
        start: 0x11CB2,
        end: 0x11CB4,
        value: "NSM",
    },
    PropertyRange {
        start: 0x11CB4,
        end: 0x11CB5,
        value: "L",
    },
    PropertyRange {
        start: 0x11CB5,
        end: 0x11CB7,
        value: "NSM",
    },
    PropertyRange {
        start: 0x11D00,
        end: 0x11D07,
        value: "L",
    },
    PropertyRange {
        start: 0x11D08,
        end: 0x11D0A,
        value: "L",
    },
    PropertyRange {
        start: 0x11D0B,
        end: 0x11D31,
        value: "L",
    },
    PropertyRange {
        start: 0x11D31,
        end: 0x11D37,
        value: "NSM",
    },
    PropertyRange {
        start: 0x11D3A,
        end: 0x11D3B,
        value: "NSM",
    },
    PropertyRange {
        start: 0x11D3C,
        end: 0x11D3E,
        value: "NSM",
    },
    PropertyRange {
        start: 0x11D3F,
        end: 0x11D46,
        value: "NSM",
    },
    PropertyRange {
        start: 0x11D46,
        end: 0x11D47,
        value: "L",
    },
    PropertyRange {
        start: 0x11D47,
        end: 0x11D48,
        value: "NSM",
    },
    PropertyRange {
        start: 0x11D50,
        end: 0x11D5A,
        value: "L",
    },
    PropertyRange {
        start: 0x11D60,
        end: 0x11D66,
        value: "L",
    },
    PropertyRange {
        start: 0x11D67,
        end: 0x11D69,
        value: "L",
    },
    PropertyRange {
        start: 0x11D6A,
        end: 0x11D8F,
        value: "L",
    },
    PropertyRange {
        start: 0x11D90,
        end: 0x11D92,
        value: "NSM",
    },
    PropertyRange {
        start: 0x11D93,
        end: 0x11D95,
        value: "L",
    },
    PropertyRange {
        start: 0x11D95,
        end: 0x11D96,
        value: "NSM",
    },
    PropertyRange {
        start: 0x11D96,
        end: 0x11D97,
        value: "L",
    },
    PropertyRange {
        start: 0x11D97,
        end: 0x11D98,
        value: "NSM",
    },
    PropertyRange {
        start: 0x11D98,
        end: 0x11D99,
        value: "L",
    },
    PropertyRange {
        start: 0x11DA0,
        end: 0x11DAA,
        value: "L",
    },
    PropertyRange {
        start: 0x11EE0,
        end: 0x11EF3,
        value: "L",
    },
    PropertyRange {
        start: 0x11EF3,
        end: 0x11EF5,
        value: "NSM",
    },
    PropertyRange {
        start: 0x11EF5,
        end: 0x11EF9,
        value: "L",
    },
    PropertyRange {
        start: 0x11F00,
        end: 0x11F02,
        value: "NSM",
    },
    PropertyRange {
        start: 0x11F02,
        end: 0x11F11,
        value: "L",
    },
    PropertyRange {
        start: 0x11F12,
        end: 0x11F36,
        value: "L",
    },
    PropertyRange {
        start: 0x11F36,
        end: 0x11F3B,
        value: "NSM",
    },
    PropertyRange {
        start: 0x11F3E,
        end: 0x11F40,
        value: "L",
    },
    PropertyRange {
        start: 0x11F40,
        end: 0x11F41,
        value: "NSM",
    },
    PropertyRange {
        start: 0x11F41,
        end: 0x11F42,
        value: "L",
    },
    PropertyRange {
        start: 0x11F42,
        end: 0x11F43,
        value: "NSM",
    },
    PropertyRange {
        start: 0x11F43,
        end: 0x11F5A,
        value: "L",
    },
    PropertyRange {
        start: 0x11F5A,
        end: 0x11F5B,
        value: "NSM",
    },
    PropertyRange {
        start: 0x11FB0,
        end: 0x11FB1,
        value: "L",
    },
    PropertyRange {
        start: 0x11FC0,
        end: 0x11FD5,
        value: "L",
    },
    PropertyRange {
        start: 0x11FD5,
        end: 0x11FDD,
        value: "ON",
    },
    PropertyRange {
        start: 0x11FDD,
        end: 0x11FE1,
        value: "ET",
    },
    PropertyRange {
        start: 0x11FE1,
        end: 0x11FF2,
        value: "ON",
    },
    PropertyRange {
        start: 0x11FFF,
        end: 0x1239A,
        value: "L",
    },
    PropertyRange {
        start: 0x12400,
        end: 0x1246F,
        value: "L",
    },
    PropertyRange {
        start: 0x12470,
        end: 0x12475,
        value: "L",
    },
    PropertyRange {
        start: 0x12480,
        end: 0x12544,
        value: "L",
    },
    PropertyRange {
        start: 0x12F90,
        end: 0x12FF3,
        value: "L",
    },
    PropertyRange {
        start: 0x13000,
        end: 0x13440,
        value: "L",
    },
    PropertyRange {
        start: 0x13440,
        end: 0x13441,
        value: "NSM",
    },
    PropertyRange {
        start: 0x13441,
        end: 0x13447,
        value: "L",
    },
    PropertyRange {
        start: 0x13447,
        end: 0x13456,
        value: "NSM",
    },
    PropertyRange {
        start: 0x13460,
        end: 0x143FB,
        value: "L",
    },
    PropertyRange {
        start: 0x14400,
        end: 0x14647,
        value: "L",
    },
    PropertyRange {
        start: 0x16100,
        end: 0x1611E,
        value: "L",
    },
    PropertyRange {
        start: 0x1611E,
        end: 0x1612A,
        value: "NSM",
    },
    PropertyRange {
        start: 0x1612A,
        end: 0x1612D,
        value: "L",
    },
    PropertyRange {
        start: 0x1612D,
        end: 0x16130,
        value: "NSM",
    },
    PropertyRange {
        start: 0x16130,
        end: 0x1613A,
        value: "L",
    },
    PropertyRange {
        start: 0x16800,
        end: 0x16A39,
        value: "L",
    },
    PropertyRange {
        start: 0x16A40,
        end: 0x16A5F,
        value: "L",
    },
    PropertyRange {
        start: 0x16A60,
        end: 0x16A6A,
        value: "L",
    },
    PropertyRange {
        start: 0x16A6E,
        end: 0x16ABF,
        value: "L",
    },
    PropertyRange {
        start: 0x16AC0,
        end: 0x16ACA,
        value: "L",
    },
    PropertyRange {
        start: 0x16AD0,
        end: 0x16AEE,
        value: "L",
    },
    PropertyRange {
        start: 0x16AF0,
        end: 0x16AF5,
        value: "NSM",
    },
    PropertyRange {
        start: 0x16AF5,
        end: 0x16AF6,
        value: "L",
    },
    PropertyRange {
        start: 0x16B00,
        end: 0x16B30,
        value: "L",
    },
    PropertyRange {
        start: 0x16B30,
        end: 0x16B37,
        value: "NSM",
    },
    PropertyRange {
        start: 0x16B37,
        end: 0x16B46,
        value: "L",
    },
    PropertyRange {
        start: 0x16B50,
        end: 0x16B5A,
        value: "L",
    },
    PropertyRange {
        start: 0x16B5B,
        end: 0x16B62,
        value: "L",
    },
    PropertyRange {
        start: 0x16B63,
        end: 0x16B78,
        value: "L",
    },
    PropertyRange {
        start: 0x16B7D,
        end: 0x16B90,
        value: "L",
    },
    PropertyRange {
        start: 0x16D40,
        end: 0x16D7A,
        value: "L",
    },
    PropertyRange {
        start: 0x16E40,
        end: 0x16E9B,
        value: "L",
    },
    PropertyRange {
        start: 0x16F00,
        end: 0x16F4B,
        value: "L",
    },
    PropertyRange {
        start: 0x16F4F,
        end: 0x16F50,
        value: "NSM",
    },
    PropertyRange {
        start: 0x16F50,
        end: 0x16F88,
        value: "L",
    },
    PropertyRange {
        start: 0x16F8F,
        end: 0x16F93,
        value: "NSM",
    },
    PropertyRange {
        start: 0x16F93,
        end: 0x16FA0,
        value: "L",
    },
    PropertyRange {
        start: 0x16FE0,
        end: 0x16FE2,
        value: "L",
    },
    PropertyRange {
        start: 0x16FE2,
        end: 0x16FE3,
        value: "ON",
    },
    PropertyRange {
        start: 0x16FE3,
        end: 0x16FE4,
        value: "L",
    },
    PropertyRange {
        start: 0x16FE4,
        end: 0x16FE5,
        value: "NSM",
    },
    PropertyRange {
        start: 0x16FF0,
        end: 0x16FF2,
        value: "L",
    },
    PropertyRange {
        start: 0x17000,
        end: 0x187F8,
        value: "L",
    },
    PropertyRange {
        start: 0x18800,
        end: 0x18CD6,
        value: "L",
    },
    PropertyRange {
        start: 0x18CFF,
        end: 0x18D09,
        value: "L",
    },
    PropertyRange {
        start: 0x1AFF0,
        end: 0x1AFF4,
        value: "L",
    },
    PropertyRange {
        start: 0x1AFF5,
        end: 0x1AFFC,
        value: "L",
    },
    PropertyRange {
        start: 0x1AFFD,
        end: 0x1AFFF,
        value: "L",
    },
    PropertyRange {
        start: 0x1B000,
        end: 0x1B123,
        value: "L",
    },
    PropertyRange {
        start: 0x1B132,
        end: 0x1B133,
        value: "L",
    },
    PropertyRange {
        start: 0x1B150,
        end: 0x1B153,
        value: "L",
    },
    PropertyRange {
        start: 0x1B155,
        end: 0x1B156,
        value: "L",
    },
    PropertyRange {
        start: 0x1B164,
        end: 0x1B168,
        value: "L",
    },
    PropertyRange {
        start: 0x1B170,
        end: 0x1B2FC,
        value: "L",
    },
    PropertyRange {
        start: 0x1BC00,
        end: 0x1BC6B,
        value: "L",
    },
    PropertyRange {
        start: 0x1BC70,
        end: 0x1BC7D,
        value: "L",
    },
    PropertyRange {
        start: 0x1BC80,
        end: 0x1BC89,
        value: "L",
    },
    PropertyRange {
        start: 0x1BC90,
        end: 0x1BC9A,
        value: "L",
    },
    PropertyRange {
        start: 0x1BC9C,
        end: 0x1BC9D,
        value: "L",
    },
    PropertyRange {
        start: 0x1BC9D,
        end: 0x1BC9F,
        value: "NSM",
    },
    PropertyRange {
        start: 0x1BC9F,
        end: 0x1BCA0,
        value: "L",
    },
    PropertyRange {
        start: 0x1BCA0,
        end: 0x1BCA4,
        value: "BN",
    },
    PropertyRange {
        start: 0x1CC00,
        end: 0x1CCD6,
        value: "ON",
    },
    PropertyRange {
        start: 0x1CCD6,
        end: 0x1CCF0,
        value: "L",
    },
    PropertyRange {
        start: 0x1CCF0,
        end: 0x1CCFA,
        value: "EN",
    },
    PropertyRange {
        start: 0x1CD00,
        end: 0x1CEB4,
        value: "ON",
    },
    PropertyRange {
        start: 0x1CF00,
        end: 0x1CF2E,
        value: "NSM",
    },
    PropertyRange {
        start: 0x1CF30,
        end: 0x1CF47,
        value: "NSM",
    },
    PropertyRange {
        start: 0x1CF50,
        end: 0x1CFC4,
        value: "L",
    },
    PropertyRange {
        start: 0x1D000,
        end: 0x1D0F6,
        value: "L",
    },
    PropertyRange {
        start: 0x1D100,
        end: 0x1D127,
        value: "L",
    },
    PropertyRange {
        start: 0x1D129,
        end: 0x1D167,
        value: "L",
    },
    PropertyRange {
        start: 0x1D167,
        end: 0x1D16A,
        value: "NSM",
    },
    PropertyRange {
        start: 0x1D16A,
        end: 0x1D173,
        value: "L",
    },
    PropertyRange {
        start: 0x1D173,
        end: 0x1D17B,
        value: "BN",
    },
    PropertyRange {
        start: 0x1D17B,
        end: 0x1D183,
        value: "NSM",
    },
    PropertyRange {
        start: 0x1D183,
        end: 0x1D185,
        value: "L",
    },
    PropertyRange {
        start: 0x1D185,
        end: 0x1D18C,
        value: "NSM",
    },
    PropertyRange {
        start: 0x1D18C,
        end: 0x1D1AA,
        value: "L",
    },
    PropertyRange {
        start: 0x1D1AA,
        end: 0x1D1AE,
        value: "NSM",
    },
    PropertyRange {
        start: 0x1D1AE,
        end: 0x1D1E9,
        value: "L",
    },
    PropertyRange {
        start: 0x1D1E9,
        end: 0x1D1EB,
        value: "ON",
    },
    PropertyRange {
        start: 0x1D200,
        end: 0x1D242,
        value: "ON",
    },
    PropertyRange {
        start: 0x1D242,
        end: 0x1D245,
        value: "NSM",
    },
    PropertyRange {
        start: 0x1D245,
        end: 0x1D246,
        value: "ON",
    },
    PropertyRange {
        start: 0x1D2C0,
        end: 0x1D2D4,
        value: "L",
    },
    PropertyRange {
        start: 0x1D2E0,
        end: 0x1D2F4,
        value: "L",
    },
    PropertyRange {
        start: 0x1D300,
        end: 0x1D357,
        value: "ON",
    },
    PropertyRange {
        start: 0x1D360,
        end: 0x1D379,
        value: "L",
    },
    PropertyRange {
        start: 0x1D400,
        end: 0x1D455,
        value: "L",
    },
    PropertyRange {
        start: 0x1D456,
        end: 0x1D49D,
        value: "L",
    },
    PropertyRange {
        start: 0x1D49E,
        end: 0x1D4A0,
        value: "L",
    },
    PropertyRange {
        start: 0x1D4A2,
        end: 0x1D4A3,
        value: "L",
    },
    PropertyRange {
        start: 0x1D4A5,
        end: 0x1D4A7,
        value: "L",
    },
    PropertyRange {
        start: 0x1D4A9,
        end: 0x1D4AD,
        value: "L",
    },
    PropertyRange {
        start: 0x1D4AE,
        end: 0x1D4BA,
        value: "L",
    },
    PropertyRange {
        start: 0x1D4BB,
        end: 0x1D4BC,
        value: "L",
    },
    PropertyRange {
        start: 0x1D4BD,
        end: 0x1D4C4,
        value: "L",
    },
    PropertyRange {
        start: 0x1D4C5,
        end: 0x1D506,
        value: "L",
    },
    PropertyRange {
        start: 0x1D507,
        end: 0x1D50B,
        value: "L",
    },
    PropertyRange {
        start: 0x1D50D,
        end: 0x1D515,
        value: "L",
    },
    PropertyRange {
        start: 0x1D516,
        end: 0x1D51D,
        value: "L",
    },
    PropertyRange {
        start: 0x1D51E,
        end: 0x1D53A,
        value: "L",
    },
    PropertyRange {
        start: 0x1D53B,
        end: 0x1D53F,
        value: "L",
    },
    PropertyRange {
        start: 0x1D540,
        end: 0x1D545,
        value: "L",
    },
    PropertyRange {
        start: 0x1D546,
        end: 0x1D547,
        value: "L",
    },
    PropertyRange {
        start: 0x1D54A,
        end: 0x1D551,
        value: "L",
    },
    PropertyRange {
        start: 0x1D552,
        end: 0x1D6A6,
        value: "L",
    },
    PropertyRange {
        start: 0x1D6A8,
        end: 0x1D6C1,
        value: "L",
    },
    PropertyRange {
        start: 0x1D6C1,
        end: 0x1D6C2,
        value: "ON",
    },
    PropertyRange {
        start: 0x1D6C2,
        end: 0x1D6DB,
        value: "L",
    },
    PropertyRange {
        start: 0x1D6DB,
        end: 0x1D6DC,
        value: "ON",
    },
    PropertyRange {
        start: 0x1D6DC,
        end: 0x1D6FB,
        value: "L",
    },
    PropertyRange {
        start: 0x1D6FB,
        end: 0x1D6FC,
        value: "ON",
    },
    PropertyRange {
        start: 0x1D6FC,
        end: 0x1D715,
        value: "L",
    },
    PropertyRange {
        start: 0x1D715,
        end: 0x1D716,
        value: "ON",
    },
    PropertyRange {
        start: 0x1D716,
        end: 0x1D735,
        value: "L",
    },
    PropertyRange {
        start: 0x1D735,
        end: 0x1D736,
        value: "ON",
    },
    PropertyRange {
        start: 0x1D736,
        end: 0x1D74F,
        value: "L",
    },
    PropertyRange {
        start: 0x1D74F,
        end: 0x1D750,
        value: "ON",
    },
    PropertyRange {
        start: 0x1D750,
        end: 0x1D76F,
        value: "L",
    },
    PropertyRange {
        start: 0x1D76F,
        end: 0x1D770,
        value: "ON",
    },
    PropertyRange {
        start: 0x1D770,
        end: 0x1D789,
        value: "L",
    },
    PropertyRange {
        start: 0x1D789,
        end: 0x1D78A,
        value: "ON",
    },
    PropertyRange {
        start: 0x1D78A,
        end: 0x1D7A9,
        value: "L",
    },
    PropertyRange {
        start: 0x1D7A9,
        end: 0x1D7AA,
        value: "ON",
    },
    PropertyRange {
        start: 0x1D7AA,
        end: 0x1D7C3,
        value: "L",
    },
    PropertyRange {
        start: 0x1D7C3,
        end: 0x1D7C4,
        value: "ON",
    },
    PropertyRange {
        start: 0x1D7C4,
        end: 0x1D7CC,
        value: "L",
    },
    PropertyRange {
        start: 0x1D7CE,
        end: 0x1D800,
        value: "EN",
    },
    PropertyRange {
        start: 0x1D800,
        end: 0x1DA00,
        value: "L",
    },
    PropertyRange {
        start: 0x1DA00,
        end: 0x1DA37,
        value: "NSM",
    },
    PropertyRange {
        start: 0x1DA37,
        end: 0x1DA3B,
        value: "L",
    },
    PropertyRange {
        start: 0x1DA3B,
        end: 0x1DA6D,
        value: "NSM",
    },
    PropertyRange {
        start: 0x1DA6D,
        end: 0x1DA75,
        value: "L",
    },
    PropertyRange {
        start: 0x1DA75,
        end: 0x1DA76,
        value: "NSM",
    },
    PropertyRange {
        start: 0x1DA76,
        end: 0x1DA84,
        value: "L",
    },
    PropertyRange {
        start: 0x1DA84,
        end: 0x1DA85,
        value: "NSM",
    },
    PropertyRange {
        start: 0x1DA85,
        end: 0x1DA8C,
        value: "L",
    },
    PropertyRange {
        start: 0x1DA9B,
        end: 0x1DAA0,
        value: "NSM",
    },
    PropertyRange {
        start: 0x1DAA1,
        end: 0x1DAB0,
        value: "NSM",
    },
    PropertyRange {
        start: 0x1DF00,
        end: 0x1DF1F,
        value: "L",
    },
    PropertyRange {
        start: 0x1DF25,
        end: 0x1DF2B,
        value: "L",
    },
    PropertyRange {
        start: 0x1E000,
        end: 0x1E007,
        value: "NSM",
    },
    PropertyRange {
        start: 0x1E008,
        end: 0x1E019,
        value: "NSM",
    },
    PropertyRange {
        start: 0x1E01B,
        end: 0x1E022,
        value: "NSM",
    },
    PropertyRange {
        start: 0x1E023,
        end: 0x1E025,
        value: "NSM",
    },
    PropertyRange {
        start: 0x1E026,
        end: 0x1E02B,
        value: "NSM",
    },
    PropertyRange {
        start: 0x1E030,
        end: 0x1E06E,
        value: "L",
    },
    PropertyRange {
        start: 0x1E08F,
        end: 0x1E090,
        value: "NSM",
    },
    PropertyRange {
        start: 0x1E100,
        end: 0x1E12D,
        value: "L",
    },
    PropertyRange {
        start: 0x1E130,
        end: 0x1E137,
        value: "NSM",
    },
    PropertyRange {
        start: 0x1E137,
        end: 0x1E13E,
        value: "L",
    },
    PropertyRange {
        start: 0x1E140,
        end: 0x1E14A,
        value: "L",
    },
    PropertyRange {
        start: 0x1E14E,
        end: 0x1E150,
        value: "L",
    },
    PropertyRange {
        start: 0x1E290,
        end: 0x1E2AE,
        value: "L",
    },
    PropertyRange {
        start: 0x1E2AE,
        end: 0x1E2AF,
        value: "NSM",
    },
    PropertyRange {
        start: 0x1E2C0,
        end: 0x1E2EC,
        value: "L",
    },
    PropertyRange {
        start: 0x1E2EC,
        end: 0x1E2F0,
        value: "NSM",
    },
    PropertyRange {
        start: 0x1E2F0,
        end: 0x1E2FA,
        value: "L",
    },
    PropertyRange {
        start: 0x1E2FF,
        end: 0x1E300,
        value: "ET",
    },
    PropertyRange {
        start: 0x1E4D0,
        end: 0x1E4EC,
        value: "L",
    },
    PropertyRange {
        start: 0x1E4EC,
        end: 0x1E4F0,
        value: "NSM",
    },
    PropertyRange {
        start: 0x1E4F0,
        end: 0x1E4FA,
        value: "L",
    },
    PropertyRange {
        start: 0x1E5D0,
        end: 0x1E5EE,
        value: "L",
    },
    PropertyRange {
        start: 0x1E5EE,
        end: 0x1E5F0,
        value: "NSM",
    },
    PropertyRange {
        start: 0x1E5F0,
        end: 0x1E5FB,
        value: "L",
    },
    PropertyRange {
        start: 0x1E5FF,
        end: 0x1E600,
        value: "L",
    },
    PropertyRange {
        start: 0x1E7E0,
        end: 0x1E7E7,
        value: "L",
    },
    PropertyRange {
        start: 0x1E7E8,
        end: 0x1E7EC,
        value: "L",
    },
    PropertyRange {
        start: 0x1E7ED,
        end: 0x1E7EF,
        value: "L",
    },
    PropertyRange {
        start: 0x1E7F0,
        end: 0x1E7FF,
        value: "L",
    },
    PropertyRange {
        start: 0x1E800,
        end: 0x1E8C5,
        value: "R",
    },
    PropertyRange {
        start: 0x1E8C7,
        end: 0x1E8D0,
        value: "R",
    },
    PropertyRange {
        start: 0x1E8D0,
        end: 0x1E8D7,
        value: "NSM",
    },
    PropertyRange {
        start: 0x1E900,
        end: 0x1E944,
        value: "R",
    },
    PropertyRange {
        start: 0x1E944,
        end: 0x1E94B,
        value: "NSM",
    },
    PropertyRange {
        start: 0x1E94B,
        end: 0x1E94C,
        value: "R",
    },
    PropertyRange {
        start: 0x1E950,
        end: 0x1E95A,
        value: "R",
    },
    PropertyRange {
        start: 0x1E95E,
        end: 0x1E960,
        value: "R",
    },
    PropertyRange {
        start: 0x1EC71,
        end: 0x1ECB5,
        value: "AL",
    },
    PropertyRange {
        start: 0x1ED01,
        end: 0x1ED3E,
        value: "AL",
    },
    PropertyRange {
        start: 0x1EE00,
        end: 0x1EE04,
        value: "AL",
    },
    PropertyRange {
        start: 0x1EE05,
        end: 0x1EE20,
        value: "AL",
    },
    PropertyRange {
        start: 0x1EE21,
        end: 0x1EE23,
        value: "AL",
    },
    PropertyRange {
        start: 0x1EE24,
        end: 0x1EE25,
        value: "AL",
    },
    PropertyRange {
        start: 0x1EE27,
        end: 0x1EE28,
        value: "AL",
    },
    PropertyRange {
        start: 0x1EE29,
        end: 0x1EE33,
        value: "AL",
    },
    PropertyRange {
        start: 0x1EE34,
        end: 0x1EE38,
        value: "AL",
    },
    PropertyRange {
        start: 0x1EE39,
        end: 0x1EE3A,
        value: "AL",
    },
    PropertyRange {
        start: 0x1EE3B,
        end: 0x1EE3C,
        value: "AL",
    },
    PropertyRange {
        start: 0x1EE42,
        end: 0x1EE43,
        value: "AL",
    },
    PropertyRange {
        start: 0x1EE47,
        end: 0x1EE48,
        value: "AL",
    },
    PropertyRange {
        start: 0x1EE49,
        end: 0x1EE4A,
        value: "AL",
    },
    PropertyRange {
        start: 0x1EE4B,
        end: 0x1EE4C,
        value: "AL",
    },
    PropertyRange {
        start: 0x1EE4D,
        end: 0x1EE50,
        value: "AL",
    },
    PropertyRange {
        start: 0x1EE51,
        end: 0x1EE53,
        value: "AL",
    },
    PropertyRange {
        start: 0x1EE54,
        end: 0x1EE55,
        value: "AL",
    },
    PropertyRange {
        start: 0x1EE57,
        end: 0x1EE58,
        value: "AL",
    },
    PropertyRange {
        start: 0x1EE59,
        end: 0x1EE5A,
        value: "AL",
    },
    PropertyRange {
        start: 0x1EE5B,
        end: 0x1EE5C,
        value: "AL",
    },
    PropertyRange {
        start: 0x1EE5D,
        end: 0x1EE5E,
        value: "AL",
    },
    PropertyRange {
        start: 0x1EE5F,
        end: 0x1EE60,
        value: "AL",
    },
    PropertyRange {
        start: 0x1EE61,
        end: 0x1EE63,
        value: "AL",
    },
    PropertyRange {
        start: 0x1EE64,
        end: 0x1EE65,
        value: "AL",
    },
    PropertyRange {
        start: 0x1EE67,
        end: 0x1EE6B,
        value: "AL",
    },
    PropertyRange {
        start: 0x1EE6C,
        end: 0x1EE73,
        value: "AL",
    },
    PropertyRange {
        start: 0x1EE74,
        end: 0x1EE78,
        value: "AL",
    },
    PropertyRange {
        start: 0x1EE79,
        end: 0x1EE7D,
        value: "AL",
    },
    PropertyRange {
        start: 0x1EE7E,
        end: 0x1EE7F,
        value: "AL",
    },
    PropertyRange {
        start: 0x1EE80,
        end: 0x1EE8A,
        value: "AL",
    },
    PropertyRange {
        start: 0x1EE8B,
        end: 0x1EE9C,
        value: "AL",
    },
    PropertyRange {
        start: 0x1EEA1,
        end: 0x1EEA4,
        value: "AL",
    },
    PropertyRange {
        start: 0x1EEA5,
        end: 0x1EEAA,
        value: "AL",
    },
    PropertyRange {
        start: 0x1EEAB,
        end: 0x1EEBC,
        value: "AL",
    },
    PropertyRange {
        start: 0x1EEF0,
        end: 0x1EEF2,
        value: "ON",
    },
    PropertyRange {
        start: 0x1F000,
        end: 0x1F02C,
        value: "ON",
    },
    PropertyRange {
        start: 0x1F030,
        end: 0x1F094,
        value: "ON",
    },
    PropertyRange {
        start: 0x1F0A0,
        end: 0x1F0AF,
        value: "ON",
    },
    PropertyRange {
        start: 0x1F0B1,
        end: 0x1F0C0,
        value: "ON",
    },
    PropertyRange {
        start: 0x1F0C1,
        end: 0x1F0D0,
        value: "ON",
    },
    PropertyRange {
        start: 0x1F0D1,
        end: 0x1F0F6,
        value: "ON",
    },
    PropertyRange {
        start: 0x1F100,
        end: 0x1F10B,
        value: "EN",
    },
    PropertyRange {
        start: 0x1F10B,
        end: 0x1F110,
        value: "ON",
    },
    PropertyRange {
        start: 0x1F110,
        end: 0x1F12F,
        value: "L",
    },
    PropertyRange {
        start: 0x1F12F,
        end: 0x1F130,
        value: "ON",
    },
    PropertyRange {
        start: 0x1F130,
        end: 0x1F16A,
        value: "L",
    },
    PropertyRange {
        start: 0x1F16A,
        end: 0x1F170,
        value: "ON",
    },
    PropertyRange {
        start: 0x1F170,
        end: 0x1F1AD,
        value: "L",
    },
    PropertyRange {
        start: 0x1F1AD,
        end: 0x1F1AE,
        value: "ON",
    },
    PropertyRange {
        start: 0x1F1E6,
        end: 0x1F203,
        value: "L",
    },
    PropertyRange {
        start: 0x1F210,
        end: 0x1F23C,
        value: "L",
    },
    PropertyRange {
        start: 0x1F240,
        end: 0x1F249,
        value: "L",
    },
    PropertyRange {
        start: 0x1F250,
        end: 0x1F252,
        value: "L",
    },
    PropertyRange {
        start: 0x1F260,
        end: 0x1F266,
        value: "ON",
    },
    PropertyRange {
        start: 0x1F300,
        end: 0x1F6D8,
        value: "ON",
    },
    PropertyRange {
        start: 0x1F6DC,
        end: 0x1F6ED,
        value: "ON",
    },
    PropertyRange {
        start: 0x1F6F0,
        end: 0x1F6FD,
        value: "ON",
    },
    PropertyRange {
        start: 0x1F700,
        end: 0x1F777,
        value: "ON",
    },
    PropertyRange {
        start: 0x1F77B,
        end: 0x1F7DA,
        value: "ON",
    },
    PropertyRange {
        start: 0x1F7E0,
        end: 0x1F7EC,
        value: "ON",
    },
    PropertyRange {
        start: 0x1F7F0,
        end: 0x1F7F1,
        value: "ON",
    },
    PropertyRange {
        start: 0x1F800,
        end: 0x1F80C,
        value: "ON",
    },
    PropertyRange {
        start: 0x1F810,
        end: 0x1F848,
        value: "ON",
    },
    PropertyRange {
        start: 0x1F850,
        end: 0x1F85A,
        value: "ON",
    },
    PropertyRange {
        start: 0x1F860,
        end: 0x1F888,
        value: "ON",
    },
    PropertyRange {
        start: 0x1F890,
        end: 0x1F8AE,
        value: "ON",
    },
    PropertyRange {
        start: 0x1F8B0,
        end: 0x1F8BC,
        value: "ON",
    },
    PropertyRange {
        start: 0x1F8C0,
        end: 0x1F8C2,
        value: "ON",
    },
    PropertyRange {
        start: 0x1F900,
        end: 0x1FA54,
        value: "ON",
    },
    PropertyRange {
        start: 0x1FA60,
        end: 0x1FA6E,
        value: "ON",
    },
    PropertyRange {
        start: 0x1FA70,
        end: 0x1FA7D,
        value: "ON",
    },
    PropertyRange {
        start: 0x1FA80,
        end: 0x1FA8A,
        value: "ON",
    },
    PropertyRange {
        start: 0x1FA8F,
        end: 0x1FAC7,
        value: "ON",
    },
    PropertyRange {
        start: 0x1FACE,
        end: 0x1FADD,
        value: "ON",
    },
    PropertyRange {
        start: 0x1FADF,
        end: 0x1FAEA,
        value: "ON",
    },
    PropertyRange {
        start: 0x1FAF0,
        end: 0x1FAF9,
        value: "ON",
    },
    PropertyRange {
        start: 0x1FB00,
        end: 0x1FB93,
        value: "ON",
    },
    PropertyRange {
        start: 0x1FB94,
        end: 0x1FBF0,
        value: "ON",
    },
    PropertyRange {
        start: 0x1FBF0,
        end: 0x1FBFA,
        value: "EN",
    },
    PropertyRange {
        start: 0x20000,
        end: 0x2A6E0,
        value: "L",
    },
    PropertyRange {
        start: 0x2A700,
        end: 0x2B73A,
        value: "L",
    },
    PropertyRange {
        start: 0x2B740,
        end: 0x2B81E,
        value: "L",
    },
    PropertyRange {
        start: 0x2B820,
        end: 0x2CEA2,
        value: "L",
    },
    PropertyRange {
        start: 0x2CEB0,
        end: 0x2EBE1,
        value: "L",
    },
    PropertyRange {
        start: 0x2EBF0,
        end: 0x2EE5E,
        value: "L",
    },
    PropertyRange {
        start: 0x2F800,
        end: 0x2FA1E,
        value: "L",
    },
    PropertyRange {
        start: 0x30000,
        end: 0x3134B,
        value: "L",
    },
    PropertyRange {
        start: 0x31350,
        end: 0x323B0,
        value: "L",
    },
    PropertyRange {
        start: 0xE0001,
        end: 0xE0002,
        value: "BN",
    },
    PropertyRange {
        start: 0xE0020,
        end: 0xE0080,
        value: "BN",
    },
    PropertyRange {
        start: 0xE0100,
        end: 0xE01F0,
        value: "NSM",
    },
    PropertyRange {
        start: 0xF0000,
        end: 0xFFFFE,
        value: "L",
    },
    PropertyRange {
        start: 0x100000,
        end: 0x10FFFE,
        value: "L",
    },
];

include!("idna2008_normalization_data.rs");
