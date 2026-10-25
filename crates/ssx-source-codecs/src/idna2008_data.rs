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

pub const CCC: &[PropertyRange<u8>] = &[
    PropertyRange {
        start: 0x300,
        end: 0x315,
        value: 230,
    },
    PropertyRange {
        start: 0x315,
        end: 0x316,
        value: 232,
    },
    PropertyRange {
        start: 0x316,
        end: 0x31A,
        value: 220,
    },
    PropertyRange {
        start: 0x31A,
        end: 0x31B,
        value: 232,
    },
    PropertyRange {
        start: 0x31B,
        end: 0x31C,
        value: 216,
    },
    PropertyRange {
        start: 0x31C,
        end: 0x321,
        value: 220,
    },
    PropertyRange {
        start: 0x321,
        end: 0x323,
        value: 202,
    },
    PropertyRange {
        start: 0x323,
        end: 0x327,
        value: 220,
    },
    PropertyRange {
        start: 0x327,
        end: 0x329,
        value: 202,
    },
    PropertyRange {
        start: 0x329,
        end: 0x334,
        value: 220,
    },
    PropertyRange {
        start: 0x334,
        end: 0x339,
        value: 1,
    },
    PropertyRange {
        start: 0x339,
        end: 0x33D,
        value: 220,
    },
    PropertyRange {
        start: 0x33D,
        end: 0x345,
        value: 230,
    },
    PropertyRange {
        start: 0x345,
        end: 0x346,
        value: 240,
    },
    PropertyRange {
        start: 0x346,
        end: 0x347,
        value: 230,
    },
    PropertyRange {
        start: 0x347,
        end: 0x34A,
        value: 220,
    },
    PropertyRange {
        start: 0x34A,
        end: 0x34D,
        value: 230,
    },
    PropertyRange {
        start: 0x34D,
        end: 0x34F,
        value: 220,
    },
    PropertyRange {
        start: 0x350,
        end: 0x353,
        value: 230,
    },
    PropertyRange {
        start: 0x353,
        end: 0x357,
        value: 220,
    },
    PropertyRange {
        start: 0x357,
        end: 0x358,
        value: 230,
    },
    PropertyRange {
        start: 0x358,
        end: 0x359,
        value: 232,
    },
    PropertyRange {
        start: 0x359,
        end: 0x35B,
        value: 220,
    },
    PropertyRange {
        start: 0x35B,
        end: 0x35C,
        value: 230,
    },
    PropertyRange {
        start: 0x35C,
        end: 0x35D,
        value: 233,
    },
    PropertyRange {
        start: 0x35D,
        end: 0x35F,
        value: 234,
    },
    PropertyRange {
        start: 0x35F,
        end: 0x360,
        value: 233,
    },
    PropertyRange {
        start: 0x360,
        end: 0x362,
        value: 234,
    },
    PropertyRange {
        start: 0x362,
        end: 0x363,
        value: 233,
    },
    PropertyRange {
        start: 0x363,
        end: 0x370,
        value: 230,
    },
    PropertyRange {
        start: 0x483,
        end: 0x488,
        value: 230,
    },
    PropertyRange {
        start: 0x591,
        end: 0x592,
        value: 220,
    },
    PropertyRange {
        start: 0x592,
        end: 0x596,
        value: 230,
    },
    PropertyRange {
        start: 0x596,
        end: 0x597,
        value: 220,
    },
    PropertyRange {
        start: 0x597,
        end: 0x59A,
        value: 230,
    },
    PropertyRange {
        start: 0x59A,
        end: 0x59B,
        value: 222,
    },
    PropertyRange {
        start: 0x59B,
        end: 0x59C,
        value: 220,
    },
    PropertyRange {
        start: 0x59C,
        end: 0x5A2,
        value: 230,
    },
    PropertyRange {
        start: 0x5A2,
        end: 0x5A8,
        value: 220,
    },
    PropertyRange {
        start: 0x5A8,
        end: 0x5AA,
        value: 230,
    },
    PropertyRange {
        start: 0x5AA,
        end: 0x5AB,
        value: 220,
    },
    PropertyRange {
        start: 0x5AB,
        end: 0x5AD,
        value: 230,
    },
    PropertyRange {
        start: 0x5AD,
        end: 0x5AE,
        value: 222,
    },
    PropertyRange {
        start: 0x5AE,
        end: 0x5AF,
        value: 228,
    },
    PropertyRange {
        start: 0x5AF,
        end: 0x5B0,
        value: 230,
    },
    PropertyRange {
        start: 0x5B0,
        end: 0x5B1,
        value: 10,
    },
    PropertyRange {
        start: 0x5B1,
        end: 0x5B2,
        value: 11,
    },
    PropertyRange {
        start: 0x5B2,
        end: 0x5B3,
        value: 12,
    },
    PropertyRange {
        start: 0x5B3,
        end: 0x5B4,
        value: 13,
    },
    PropertyRange {
        start: 0x5B4,
        end: 0x5B5,
        value: 14,
    },
    PropertyRange {
        start: 0x5B5,
        end: 0x5B6,
        value: 15,
    },
    PropertyRange {
        start: 0x5B6,
        end: 0x5B7,
        value: 16,
    },
    PropertyRange {
        start: 0x5B7,
        end: 0x5B8,
        value: 17,
    },
    PropertyRange {
        start: 0x5B8,
        end: 0x5B9,
        value: 18,
    },
    PropertyRange {
        start: 0x5B9,
        end: 0x5BB,
        value: 19,
    },
    PropertyRange {
        start: 0x5BB,
        end: 0x5BC,
        value: 20,
    },
    PropertyRange {
        start: 0x5BC,
        end: 0x5BD,
        value: 21,
    },
    PropertyRange {
        start: 0x5BD,
        end: 0x5BE,
        value: 22,
    },
    PropertyRange {
        start: 0x5BF,
        end: 0x5C0,
        value: 23,
    },
    PropertyRange {
        start: 0x5C1,
        end: 0x5C2,
        value: 24,
    },
    PropertyRange {
        start: 0x5C2,
        end: 0x5C3,
        value: 25,
    },
    PropertyRange {
        start: 0x5C4,
        end: 0x5C5,
        value: 230,
    },
    PropertyRange {
        start: 0x5C5,
        end: 0x5C6,
        value: 220,
    },
    PropertyRange {
        start: 0x5C7,
        end: 0x5C8,
        value: 18,
    },
    PropertyRange {
        start: 0x610,
        end: 0x618,
        value: 230,
    },
    PropertyRange {
        start: 0x618,
        end: 0x619,
        value: 30,
    },
    PropertyRange {
        start: 0x619,
        end: 0x61A,
        value: 31,
    },
    PropertyRange {
        start: 0x61A,
        end: 0x61B,
        value: 32,
    },
    PropertyRange {
        start: 0x64B,
        end: 0x64C,
        value: 27,
    },
    PropertyRange {
        start: 0x64C,
        end: 0x64D,
        value: 28,
    },
    PropertyRange {
        start: 0x64D,
        end: 0x64E,
        value: 29,
    },
    PropertyRange {
        start: 0x64E,
        end: 0x64F,
        value: 30,
    },
    PropertyRange {
        start: 0x64F,
        end: 0x650,
        value: 31,
    },
    PropertyRange {
        start: 0x650,
        end: 0x651,
        value: 32,
    },
    PropertyRange {
        start: 0x651,
        end: 0x652,
        value: 33,
    },
    PropertyRange {
        start: 0x652,
        end: 0x653,
        value: 34,
    },
    PropertyRange {
        start: 0x653,
        end: 0x655,
        value: 230,
    },
    PropertyRange {
        start: 0x655,
        end: 0x657,
        value: 220,
    },
    PropertyRange {
        start: 0x657,
        end: 0x65C,
        value: 230,
    },
    PropertyRange {
        start: 0x65C,
        end: 0x65D,
        value: 220,
    },
    PropertyRange {
        start: 0x65D,
        end: 0x65F,
        value: 230,
    },
    PropertyRange {
        start: 0x65F,
        end: 0x660,
        value: 220,
    },
    PropertyRange {
        start: 0x670,
        end: 0x671,
        value: 35,
    },
    PropertyRange {
        start: 0x6D6,
        end: 0x6DD,
        value: 230,
    },
    PropertyRange {
        start: 0x6DF,
        end: 0x6E3,
        value: 230,
    },
    PropertyRange {
        start: 0x6E3,
        end: 0x6E4,
        value: 220,
    },
    PropertyRange {
        start: 0x6E4,
        end: 0x6E5,
        value: 230,
    },
    PropertyRange {
        start: 0x6E7,
        end: 0x6E9,
        value: 230,
    },
    PropertyRange {
        start: 0x6EA,
        end: 0x6EB,
        value: 220,
    },
    PropertyRange {
        start: 0x6EB,
        end: 0x6ED,
        value: 230,
    },
    PropertyRange {
        start: 0x6ED,
        end: 0x6EE,
        value: 220,
    },
    PropertyRange {
        start: 0x711,
        end: 0x712,
        value: 36,
    },
    PropertyRange {
        start: 0x730,
        end: 0x731,
        value: 230,
    },
    PropertyRange {
        start: 0x731,
        end: 0x732,
        value: 220,
    },
    PropertyRange {
        start: 0x732,
        end: 0x734,
        value: 230,
    },
    PropertyRange {
        start: 0x734,
        end: 0x735,
        value: 220,
    },
    PropertyRange {
        start: 0x735,
        end: 0x737,
        value: 230,
    },
    PropertyRange {
        start: 0x737,
        end: 0x73A,
        value: 220,
    },
    PropertyRange {
        start: 0x73A,
        end: 0x73B,
        value: 230,
    },
    PropertyRange {
        start: 0x73B,
        end: 0x73D,
        value: 220,
    },
    PropertyRange {
        start: 0x73D,
        end: 0x73E,
        value: 230,
    },
    PropertyRange {
        start: 0x73E,
        end: 0x73F,
        value: 220,
    },
    PropertyRange {
        start: 0x73F,
        end: 0x742,
        value: 230,
    },
    PropertyRange {
        start: 0x742,
        end: 0x743,
        value: 220,
    },
    PropertyRange {
        start: 0x743,
        end: 0x744,
        value: 230,
    },
    PropertyRange {
        start: 0x744,
        end: 0x745,
        value: 220,
    },
    PropertyRange {
        start: 0x745,
        end: 0x746,
        value: 230,
    },
    PropertyRange {
        start: 0x746,
        end: 0x747,
        value: 220,
    },
    PropertyRange {
        start: 0x747,
        end: 0x748,
        value: 230,
    },
    PropertyRange {
        start: 0x748,
        end: 0x749,
        value: 220,
    },
    PropertyRange {
        start: 0x749,
        end: 0x74B,
        value: 230,
    },
    PropertyRange {
        start: 0x7EB,
        end: 0x7F2,
        value: 230,
    },
    PropertyRange {
        start: 0x7F2,
        end: 0x7F3,
        value: 220,
    },
    PropertyRange {
        start: 0x7F3,
        end: 0x7F4,
        value: 230,
    },
    PropertyRange {
        start: 0x7FD,
        end: 0x7FE,
        value: 220,
    },
    PropertyRange {
        start: 0x816,
        end: 0x81A,
        value: 230,
    },
    PropertyRange {
        start: 0x81B,
        end: 0x824,
        value: 230,
    },
    PropertyRange {
        start: 0x825,
        end: 0x828,
        value: 230,
    },
    PropertyRange {
        start: 0x829,
        end: 0x82E,
        value: 230,
    },
    PropertyRange {
        start: 0x859,
        end: 0x85C,
        value: 220,
    },
    PropertyRange {
        start: 0x897,
        end: 0x899,
        value: 230,
    },
    PropertyRange {
        start: 0x899,
        end: 0x89C,
        value: 220,
    },
    PropertyRange {
        start: 0x89C,
        end: 0x8A0,
        value: 230,
    },
    PropertyRange {
        start: 0x8CA,
        end: 0x8CF,
        value: 230,
    },
    PropertyRange {
        start: 0x8CF,
        end: 0x8D4,
        value: 220,
    },
    PropertyRange {
        start: 0x8D4,
        end: 0x8E2,
        value: 230,
    },
    PropertyRange {
        start: 0x8E3,
        end: 0x8E4,
        value: 220,
    },
    PropertyRange {
        start: 0x8E4,
        end: 0x8E6,
        value: 230,
    },
    PropertyRange {
        start: 0x8E6,
        end: 0x8E7,
        value: 220,
    },
    PropertyRange {
        start: 0x8E7,
        end: 0x8E9,
        value: 230,
    },
    PropertyRange {
        start: 0x8E9,
        end: 0x8EA,
        value: 220,
    },
    PropertyRange {
        start: 0x8EA,
        end: 0x8ED,
        value: 230,
    },
    PropertyRange {
        start: 0x8ED,
        end: 0x8F0,
        value: 220,
    },
    PropertyRange {
        start: 0x8F0,
        end: 0x8F1,
        value: 27,
    },
    PropertyRange {
        start: 0x8F1,
        end: 0x8F2,
        value: 28,
    },
    PropertyRange {
        start: 0x8F2,
        end: 0x8F3,
        value: 29,
    },
    PropertyRange {
        start: 0x8F3,
        end: 0x8F6,
        value: 230,
    },
    PropertyRange {
        start: 0x8F6,
        end: 0x8F7,
        value: 220,
    },
    PropertyRange {
        start: 0x8F7,
        end: 0x8F9,
        value: 230,
    },
    PropertyRange {
        start: 0x8F9,
        end: 0x8FB,
        value: 220,
    },
    PropertyRange {
        start: 0x8FB,
        end: 0x900,
        value: 230,
    },
    PropertyRange {
        start: 0x93C,
        end: 0x93D,
        value: 7,
    },
    PropertyRange {
        start: 0x94D,
        end: 0x94E,
        value: 9,
    },
    PropertyRange {
        start: 0x951,
        end: 0x952,
        value: 230,
    },
    PropertyRange {
        start: 0x952,
        end: 0x953,
        value: 220,
    },
    PropertyRange {
        start: 0x953,
        end: 0x955,
        value: 230,
    },
    PropertyRange {
        start: 0x9BC,
        end: 0x9BD,
        value: 7,
    },
    PropertyRange {
        start: 0x9CD,
        end: 0x9CE,
        value: 9,
    },
    PropertyRange {
        start: 0x9FE,
        end: 0x9FF,
        value: 230,
    },
    PropertyRange {
        start: 0xA3C,
        end: 0xA3D,
        value: 7,
    },
    PropertyRange {
        start: 0xA4D,
        end: 0xA4E,
        value: 9,
    },
    PropertyRange {
        start: 0xABC,
        end: 0xABD,
        value: 7,
    },
    PropertyRange {
        start: 0xACD,
        end: 0xACE,
        value: 9,
    },
    PropertyRange {
        start: 0xB3C,
        end: 0xB3D,
        value: 7,
    },
    PropertyRange {
        start: 0xB4D,
        end: 0xB4E,
        value: 9,
    },
    PropertyRange {
        start: 0xBCD,
        end: 0xBCE,
        value: 9,
    },
    PropertyRange {
        start: 0xC3C,
        end: 0xC3D,
        value: 7,
    },
    PropertyRange {
        start: 0xC4D,
        end: 0xC4E,
        value: 9,
    },
    PropertyRange {
        start: 0xC55,
        end: 0xC56,
        value: 84,
    },
    PropertyRange {
        start: 0xC56,
        end: 0xC57,
        value: 91,
    },
    PropertyRange {
        start: 0xCBC,
        end: 0xCBD,
        value: 7,
    },
    PropertyRange {
        start: 0xCCD,
        end: 0xCCE,
        value: 9,
    },
    PropertyRange {
        start: 0xD3B,
        end: 0xD3D,
        value: 9,
    },
    PropertyRange {
        start: 0xD4D,
        end: 0xD4E,
        value: 9,
    },
    PropertyRange {
        start: 0xDCA,
        end: 0xDCB,
        value: 9,
    },
    PropertyRange {
        start: 0xE38,
        end: 0xE3A,
        value: 103,
    },
    PropertyRange {
        start: 0xE3A,
        end: 0xE3B,
        value: 9,
    },
    PropertyRange {
        start: 0xE48,
        end: 0xE4C,
        value: 107,
    },
    PropertyRange {
        start: 0xEB8,
        end: 0xEBA,
        value: 118,
    },
    PropertyRange {
        start: 0xEBA,
        end: 0xEBB,
        value: 9,
    },
    PropertyRange {
        start: 0xEC8,
        end: 0xECC,
        value: 122,
    },
    PropertyRange {
        start: 0xF18,
        end: 0xF1A,
        value: 220,
    },
    PropertyRange {
        start: 0xF35,
        end: 0xF36,
        value: 220,
    },
    PropertyRange {
        start: 0xF37,
        end: 0xF38,
        value: 220,
    },
    PropertyRange {
        start: 0xF39,
        end: 0xF3A,
        value: 216,
    },
    PropertyRange {
        start: 0xF71,
        end: 0xF72,
        value: 129,
    },
    PropertyRange {
        start: 0xF72,
        end: 0xF73,
        value: 130,
    },
    PropertyRange {
        start: 0xF74,
        end: 0xF75,
        value: 132,
    },
    PropertyRange {
        start: 0xF7A,
        end: 0xF7E,
        value: 130,
    },
    PropertyRange {
        start: 0xF80,
        end: 0xF81,
        value: 130,
    },
    PropertyRange {
        start: 0xF82,
        end: 0xF84,
        value: 230,
    },
    PropertyRange {
        start: 0xF84,
        end: 0xF85,
        value: 9,
    },
    PropertyRange {
        start: 0xF86,
        end: 0xF88,
        value: 230,
    },
    PropertyRange {
        start: 0xFC6,
        end: 0xFC7,
        value: 220,
    },
    PropertyRange {
        start: 0x1037,
        end: 0x1038,
        value: 7,
    },
    PropertyRange {
        start: 0x1039,
        end: 0x103B,
        value: 9,
    },
    PropertyRange {
        start: 0x108D,
        end: 0x108E,
        value: 220,
    },
    PropertyRange {
        start: 0x135D,
        end: 0x1360,
        value: 230,
    },
    PropertyRange {
        start: 0x1714,
        end: 0x1716,
        value: 9,
    },
    PropertyRange {
        start: 0x1734,
        end: 0x1735,
        value: 9,
    },
    PropertyRange {
        start: 0x17D2,
        end: 0x17D3,
        value: 9,
    },
    PropertyRange {
        start: 0x17DD,
        end: 0x17DE,
        value: 230,
    },
    PropertyRange {
        start: 0x18A9,
        end: 0x18AA,
        value: 228,
    },
    PropertyRange {
        start: 0x1939,
        end: 0x193A,
        value: 222,
    },
    PropertyRange {
        start: 0x193A,
        end: 0x193B,
        value: 230,
    },
    PropertyRange {
        start: 0x193B,
        end: 0x193C,
        value: 220,
    },
    PropertyRange {
        start: 0x1A17,
        end: 0x1A18,
        value: 230,
    },
    PropertyRange {
        start: 0x1A18,
        end: 0x1A19,
        value: 220,
    },
    PropertyRange {
        start: 0x1A60,
        end: 0x1A61,
        value: 9,
    },
    PropertyRange {
        start: 0x1A75,
        end: 0x1A7D,
        value: 230,
    },
    PropertyRange {
        start: 0x1A7F,
        end: 0x1A80,
        value: 220,
    },
    PropertyRange {
        start: 0x1AB0,
        end: 0x1AB5,
        value: 230,
    },
    PropertyRange {
        start: 0x1AB5,
        end: 0x1ABB,
        value: 220,
    },
    PropertyRange {
        start: 0x1ABB,
        end: 0x1ABD,
        value: 230,
    },
    PropertyRange {
        start: 0x1ABD,
        end: 0x1ABE,
        value: 220,
    },
    PropertyRange {
        start: 0x1ABF,
        end: 0x1AC1,
        value: 220,
    },
    PropertyRange {
        start: 0x1AC1,
        end: 0x1AC3,
        value: 230,
    },
    PropertyRange {
        start: 0x1AC3,
        end: 0x1AC5,
        value: 220,
    },
    PropertyRange {
        start: 0x1AC5,
        end: 0x1ACA,
        value: 230,
    },
    PropertyRange {
        start: 0x1ACA,
        end: 0x1ACB,
        value: 220,
    },
    PropertyRange {
        start: 0x1ACB,
        end: 0x1ACF,
        value: 230,
    },
    PropertyRange {
        start: 0x1B34,
        end: 0x1B35,
        value: 7,
    },
    PropertyRange {
        start: 0x1B44,
        end: 0x1B45,
        value: 9,
    },
    PropertyRange {
        start: 0x1B6B,
        end: 0x1B6C,
        value: 230,
    },
    PropertyRange {
        start: 0x1B6C,
        end: 0x1B6D,
        value: 220,
    },
    PropertyRange {
        start: 0x1B6D,
        end: 0x1B74,
        value: 230,
    },
    PropertyRange {
        start: 0x1BAA,
        end: 0x1BAC,
        value: 9,
    },
    PropertyRange {
        start: 0x1BE6,
        end: 0x1BE7,
        value: 7,
    },
    PropertyRange {
        start: 0x1BF2,
        end: 0x1BF4,
        value: 9,
    },
    PropertyRange {
        start: 0x1C37,
        end: 0x1C38,
        value: 7,
    },
    PropertyRange {
        start: 0x1CD0,
        end: 0x1CD3,
        value: 230,
    },
    PropertyRange {
        start: 0x1CD4,
        end: 0x1CD5,
        value: 1,
    },
    PropertyRange {
        start: 0x1CD5,
        end: 0x1CDA,
        value: 220,
    },
    PropertyRange {
        start: 0x1CDA,
        end: 0x1CDC,
        value: 230,
    },
    PropertyRange {
        start: 0x1CDC,
        end: 0x1CE0,
        value: 220,
    },
    PropertyRange {
        start: 0x1CE0,
        end: 0x1CE1,
        value: 230,
    },
    PropertyRange {
        start: 0x1CE2,
        end: 0x1CE9,
        value: 1,
    },
    PropertyRange {
        start: 0x1CED,
        end: 0x1CEE,
        value: 220,
    },
    PropertyRange {
        start: 0x1CF4,
        end: 0x1CF5,
        value: 230,
    },
    PropertyRange {
        start: 0x1CF8,
        end: 0x1CFA,
        value: 230,
    },
    PropertyRange {
        start: 0x1DC0,
        end: 0x1DC2,
        value: 230,
    },
    PropertyRange {
        start: 0x1DC2,
        end: 0x1DC3,
        value: 220,
    },
    PropertyRange {
        start: 0x1DC3,
        end: 0x1DCA,
        value: 230,
    },
    PropertyRange {
        start: 0x1DCA,
        end: 0x1DCB,
        value: 220,
    },
    PropertyRange {
        start: 0x1DCB,
        end: 0x1DCD,
        value: 230,
    },
    PropertyRange {
        start: 0x1DCD,
        end: 0x1DCE,
        value: 234,
    },
    PropertyRange {
        start: 0x1DCE,
        end: 0x1DCF,
        value: 214,
    },
    PropertyRange {
        start: 0x1DCF,
        end: 0x1DD0,
        value: 220,
    },
    PropertyRange {
        start: 0x1DD0,
        end: 0x1DD1,
        value: 202,
    },
    PropertyRange {
        start: 0x1DD1,
        end: 0x1DF6,
        value: 230,
    },
    PropertyRange {
        start: 0x1DF6,
        end: 0x1DF7,
        value: 232,
    },
    PropertyRange {
        start: 0x1DF7,
        end: 0x1DF9,
        value: 228,
    },
    PropertyRange {
        start: 0x1DF9,
        end: 0x1DFA,
        value: 220,
    },
    PropertyRange {
        start: 0x1DFA,
        end: 0x1DFB,
        value: 218,
    },
    PropertyRange {
        start: 0x1DFB,
        end: 0x1DFC,
        value: 230,
    },
    PropertyRange {
        start: 0x1DFC,
        end: 0x1DFD,
        value: 233,
    },
    PropertyRange {
        start: 0x1DFD,
        end: 0x1DFE,
        value: 220,
    },
    PropertyRange {
        start: 0x1DFE,
        end: 0x1DFF,
        value: 230,
    },
    PropertyRange {
        start: 0x1DFF,
        end: 0x1E00,
        value: 220,
    },
    PropertyRange {
        start: 0x20D0,
        end: 0x20D2,
        value: 230,
    },
    PropertyRange {
        start: 0x20D2,
        end: 0x20D4,
        value: 1,
    },
    PropertyRange {
        start: 0x20D4,
        end: 0x20D8,
        value: 230,
    },
    PropertyRange {
        start: 0x20D8,
        end: 0x20DB,
        value: 1,
    },
    PropertyRange {
        start: 0x20DB,
        end: 0x20DD,
        value: 230,
    },
    PropertyRange {
        start: 0x20E1,
        end: 0x20E2,
        value: 230,
    },
    PropertyRange {
        start: 0x20E5,
        end: 0x20E7,
        value: 1,
    },
    PropertyRange {
        start: 0x20E7,
        end: 0x20E8,
        value: 230,
    },
    PropertyRange {
        start: 0x20E8,
        end: 0x20E9,
        value: 220,
    },
    PropertyRange {
        start: 0x20E9,
        end: 0x20EA,
        value: 230,
    },
    PropertyRange {
        start: 0x20EA,
        end: 0x20EC,
        value: 1,
    },
    PropertyRange {
        start: 0x20EC,
        end: 0x20F0,
        value: 220,
    },
    PropertyRange {
        start: 0x20F0,
        end: 0x20F1,
        value: 230,
    },
    PropertyRange {
        start: 0x2CEF,
        end: 0x2CF2,
        value: 230,
    },
    PropertyRange {
        start: 0x2D7F,
        end: 0x2D80,
        value: 9,
    },
    PropertyRange {
        start: 0x2DE0,
        end: 0x2E00,
        value: 230,
    },
    PropertyRange {
        start: 0x302A,
        end: 0x302B,
        value: 218,
    },
    PropertyRange {
        start: 0x302B,
        end: 0x302C,
        value: 228,
    },
    PropertyRange {
        start: 0x302C,
        end: 0x302D,
        value: 232,
    },
    PropertyRange {
        start: 0x302D,
        end: 0x302E,
        value: 222,
    },
    PropertyRange {
        start: 0x302E,
        end: 0x3030,
        value: 224,
    },
    PropertyRange {
        start: 0x3099,
        end: 0x309B,
        value: 8,
    },
    PropertyRange {
        start: 0xA66F,
        end: 0xA670,
        value: 230,
    },
    PropertyRange {
        start: 0xA674,
        end: 0xA67E,
        value: 230,
    },
    PropertyRange {
        start: 0xA69E,
        end: 0xA6A0,
        value: 230,
    },
    PropertyRange {
        start: 0xA6F0,
        end: 0xA6F2,
        value: 230,
    },
    PropertyRange {
        start: 0xA806,
        end: 0xA807,
        value: 9,
    },
    PropertyRange {
        start: 0xA82C,
        end: 0xA82D,
        value: 9,
    },
    PropertyRange {
        start: 0xA8C4,
        end: 0xA8C5,
        value: 9,
    },
    PropertyRange {
        start: 0xA8E0,
        end: 0xA8F2,
        value: 230,
    },
    PropertyRange {
        start: 0xA92B,
        end: 0xA92E,
        value: 220,
    },
    PropertyRange {
        start: 0xA953,
        end: 0xA954,
        value: 9,
    },
    PropertyRange {
        start: 0xA9B3,
        end: 0xA9B4,
        value: 7,
    },
    PropertyRange {
        start: 0xA9C0,
        end: 0xA9C1,
        value: 9,
    },
    PropertyRange {
        start: 0xAAB0,
        end: 0xAAB1,
        value: 230,
    },
    PropertyRange {
        start: 0xAAB2,
        end: 0xAAB4,
        value: 230,
    },
    PropertyRange {
        start: 0xAAB4,
        end: 0xAAB5,
        value: 220,
    },
    PropertyRange {
        start: 0xAAB7,
        end: 0xAAB9,
        value: 230,
    },
    PropertyRange {
        start: 0xAABE,
        end: 0xAAC0,
        value: 230,
    },
    PropertyRange {
        start: 0xAAC1,
        end: 0xAAC2,
        value: 230,
    },
    PropertyRange {
        start: 0xAAF6,
        end: 0xAAF7,
        value: 9,
    },
    PropertyRange {
        start: 0xABED,
        end: 0xABEE,
        value: 9,
    },
    PropertyRange {
        start: 0xFB1E,
        end: 0xFB1F,
        value: 26,
    },
    PropertyRange {
        start: 0xFE20,
        end: 0xFE27,
        value: 230,
    },
    PropertyRange {
        start: 0xFE27,
        end: 0xFE2E,
        value: 220,
    },
    PropertyRange {
        start: 0xFE2E,
        end: 0xFE30,
        value: 230,
    },
    PropertyRange {
        start: 0x101FD,
        end: 0x101FE,
        value: 220,
    },
    PropertyRange {
        start: 0x102E0,
        end: 0x102E1,
        value: 220,
    },
    PropertyRange {
        start: 0x10376,
        end: 0x1037B,
        value: 230,
    },
    PropertyRange {
        start: 0x10A0D,
        end: 0x10A0E,
        value: 220,
    },
    PropertyRange {
        start: 0x10A0F,
        end: 0x10A10,
        value: 230,
    },
    PropertyRange {
        start: 0x10A38,
        end: 0x10A39,
        value: 230,
    },
    PropertyRange {
        start: 0x10A39,
        end: 0x10A3A,
        value: 1,
    },
    PropertyRange {
        start: 0x10A3A,
        end: 0x10A3B,
        value: 220,
    },
    PropertyRange {
        start: 0x10A3F,
        end: 0x10A40,
        value: 9,
    },
    PropertyRange {
        start: 0x10AE5,
        end: 0x10AE6,
        value: 230,
    },
    PropertyRange {
        start: 0x10AE6,
        end: 0x10AE7,
        value: 220,
    },
    PropertyRange {
        start: 0x10D24,
        end: 0x10D28,
        value: 230,
    },
    PropertyRange {
        start: 0x10D69,
        end: 0x10D6E,
        value: 230,
    },
    PropertyRange {
        start: 0x10EAB,
        end: 0x10EAD,
        value: 230,
    },
    PropertyRange {
        start: 0x10EFD,
        end: 0x10F00,
        value: 220,
    },
    PropertyRange {
        start: 0x10F46,
        end: 0x10F48,
        value: 220,
    },
    PropertyRange {
        start: 0x10F48,
        end: 0x10F4B,
        value: 230,
    },
    PropertyRange {
        start: 0x10F4B,
        end: 0x10F4C,
        value: 220,
    },
    PropertyRange {
        start: 0x10F4C,
        end: 0x10F4D,
        value: 230,
    },
    PropertyRange {
        start: 0x10F4D,
        end: 0x10F51,
        value: 220,
    },
    PropertyRange {
        start: 0x10F82,
        end: 0x10F83,
        value: 230,
    },
    PropertyRange {
        start: 0x10F83,
        end: 0x10F84,
        value: 220,
    },
    PropertyRange {
        start: 0x10F84,
        end: 0x10F85,
        value: 230,
    },
    PropertyRange {
        start: 0x10F85,
        end: 0x10F86,
        value: 220,
    },
    PropertyRange {
        start: 0x11046,
        end: 0x11047,
        value: 9,
    },
    PropertyRange {
        start: 0x11070,
        end: 0x11071,
        value: 9,
    },
    PropertyRange {
        start: 0x1107F,
        end: 0x11080,
        value: 9,
    },
    PropertyRange {
        start: 0x110B9,
        end: 0x110BA,
        value: 9,
    },
    PropertyRange {
        start: 0x110BA,
        end: 0x110BB,
        value: 7,
    },
    PropertyRange {
        start: 0x11100,
        end: 0x11103,
        value: 230,
    },
    PropertyRange {
        start: 0x11133,
        end: 0x11135,
        value: 9,
    },
    PropertyRange {
        start: 0x11173,
        end: 0x11174,
        value: 7,
    },
    PropertyRange {
        start: 0x111C0,
        end: 0x111C1,
        value: 9,
    },
    PropertyRange {
        start: 0x111CA,
        end: 0x111CB,
        value: 7,
    },
    PropertyRange {
        start: 0x11235,
        end: 0x11236,
        value: 9,
    },
    PropertyRange {
        start: 0x11236,
        end: 0x11237,
        value: 7,
    },
    PropertyRange {
        start: 0x112E9,
        end: 0x112EA,
        value: 7,
    },
    PropertyRange {
        start: 0x112EA,
        end: 0x112EB,
        value: 9,
    },
    PropertyRange {
        start: 0x1133B,
        end: 0x1133D,
        value: 7,
    },
    PropertyRange {
        start: 0x1134D,
        end: 0x1134E,
        value: 9,
    },
    PropertyRange {
        start: 0x11366,
        end: 0x1136D,
        value: 230,
    },
    PropertyRange {
        start: 0x11370,
        end: 0x11375,
        value: 230,
    },
    PropertyRange {
        start: 0x113CE,
        end: 0x113D1,
        value: 9,
    },
    PropertyRange {
        start: 0x11442,
        end: 0x11443,
        value: 9,
    },
    PropertyRange {
        start: 0x11446,
        end: 0x11447,
        value: 7,
    },
    PropertyRange {
        start: 0x1145E,
        end: 0x1145F,
        value: 230,
    },
    PropertyRange {
        start: 0x114C2,
        end: 0x114C3,
        value: 9,
    },
    PropertyRange {
        start: 0x114C3,
        end: 0x114C4,
        value: 7,
    },
    PropertyRange {
        start: 0x115BF,
        end: 0x115C0,
        value: 9,
    },
    PropertyRange {
        start: 0x115C0,
        end: 0x115C1,
        value: 7,
    },
    PropertyRange {
        start: 0x1163F,
        end: 0x11640,
        value: 9,
    },
    PropertyRange {
        start: 0x116B6,
        end: 0x116B7,
        value: 9,
    },
    PropertyRange {
        start: 0x116B7,
        end: 0x116B8,
        value: 7,
    },
    PropertyRange {
        start: 0x1172B,
        end: 0x1172C,
        value: 9,
    },
    PropertyRange {
        start: 0x11839,
        end: 0x1183A,
        value: 9,
    },
    PropertyRange {
        start: 0x1183A,
        end: 0x1183B,
        value: 7,
    },
    PropertyRange {
        start: 0x1193D,
        end: 0x1193F,
        value: 9,
    },
    PropertyRange {
        start: 0x11943,
        end: 0x11944,
        value: 7,
    },
    PropertyRange {
        start: 0x119E0,
        end: 0x119E1,
        value: 9,
    },
    PropertyRange {
        start: 0x11A34,
        end: 0x11A35,
        value: 9,
    },
    PropertyRange {
        start: 0x11A47,
        end: 0x11A48,
        value: 9,
    },
    PropertyRange {
        start: 0x11A99,
        end: 0x11A9A,
        value: 9,
    },
    PropertyRange {
        start: 0x11C3F,
        end: 0x11C40,
        value: 9,
    },
    PropertyRange {
        start: 0x11D42,
        end: 0x11D43,
        value: 7,
    },
    PropertyRange {
        start: 0x11D44,
        end: 0x11D46,
        value: 9,
    },
    PropertyRange {
        start: 0x11D97,
        end: 0x11D98,
        value: 9,
    },
    PropertyRange {
        start: 0x11F41,
        end: 0x11F43,
        value: 9,
    },
    PropertyRange {
        start: 0x1612F,
        end: 0x16130,
        value: 9,
    },
    PropertyRange {
        start: 0x16AF0,
        end: 0x16AF5,
        value: 1,
    },
    PropertyRange {
        start: 0x16B30,
        end: 0x16B37,
        value: 230,
    },
    PropertyRange {
        start: 0x16FF0,
        end: 0x16FF2,
        value: 6,
    },
    PropertyRange {
        start: 0x1BC9E,
        end: 0x1BC9F,
        value: 1,
    },
    PropertyRange {
        start: 0x1D165,
        end: 0x1D167,
        value: 216,
    },
    PropertyRange {
        start: 0x1D167,
        end: 0x1D16A,
        value: 1,
    },
    PropertyRange {
        start: 0x1D16D,
        end: 0x1D16E,
        value: 226,
    },
    PropertyRange {
        start: 0x1D16E,
        end: 0x1D173,
        value: 216,
    },
    PropertyRange {
        start: 0x1D17B,
        end: 0x1D183,
        value: 220,
    },
    PropertyRange {
        start: 0x1D185,
        end: 0x1D18A,
        value: 230,
    },
    PropertyRange {
        start: 0x1D18A,
        end: 0x1D18C,
        value: 220,
    },
    PropertyRange {
        start: 0x1D1AA,
        end: 0x1D1AE,
        value: 230,
    },
    PropertyRange {
        start: 0x1D242,
        end: 0x1D245,
        value: 230,
    },
    PropertyRange {
        start: 0x1E000,
        end: 0x1E007,
        value: 230,
    },
    PropertyRange {
        start: 0x1E008,
        end: 0x1E019,
        value: 230,
    },
    PropertyRange {
        start: 0x1E01B,
        end: 0x1E022,
        value: 230,
    },
    PropertyRange {
        start: 0x1E023,
        end: 0x1E025,
        value: 230,
    },
    PropertyRange {
        start: 0x1E026,
        end: 0x1E02B,
        value: 230,
    },
    PropertyRange {
        start: 0x1E08F,
        end: 0x1E090,
        value: 230,
    },
    PropertyRange {
        start: 0x1E130,
        end: 0x1E137,
        value: 230,
    },
    PropertyRange {
        start: 0x1E2AE,
        end: 0x1E2AF,
        value: 230,
    },
    PropertyRange {
        start: 0x1E2EC,
        end: 0x1E2F0,
        value: 230,
    },
    PropertyRange {
        start: 0x1E4EC,
        end: 0x1E4EE,
        value: 232,
    },
    PropertyRange {
        start: 0x1E4EE,
        end: 0x1E4EF,
        value: 220,
    },
    PropertyRange {
        start: 0x1E4EF,
        end: 0x1E4F0,
        value: 230,
    },
    PropertyRange {
        start: 0x1E5EE,
        end: 0x1E5EF,
        value: 230,
    },
    PropertyRange {
        start: 0x1E5EF,
        end: 0x1E5F0,
        value: 220,
    },
    PropertyRange {
        start: 0x1E8D0,
        end: 0x1E8D7,
        value: 220,
    },
    PropertyRange {
        start: 0x1E944,
        end: 0x1E94A,
        value: 230,
    },
    PropertyRange {
        start: 0x1E94A,
        end: 0x1E94B,
        value: 7,
    },
];

pub const DECIMAL: &[PropertyRange<u8>] = &[
    PropertyRange {
        start: 0x30,
        end: 0x31,
        value: 0,
    },
    PropertyRange {
        start: 0x31,
        end: 0x32,
        value: 1,
    },
    PropertyRange {
        start: 0x32,
        end: 0x33,
        value: 2,
    },
    PropertyRange {
        start: 0x33,
        end: 0x34,
        value: 3,
    },
    PropertyRange {
        start: 0x34,
        end: 0x35,
        value: 4,
    },
    PropertyRange {
        start: 0x35,
        end: 0x36,
        value: 5,
    },
    PropertyRange {
        start: 0x36,
        end: 0x37,
        value: 6,
    },
    PropertyRange {
        start: 0x37,
        end: 0x38,
        value: 7,
    },
    PropertyRange {
        start: 0x38,
        end: 0x39,
        value: 8,
    },
    PropertyRange {
        start: 0x39,
        end: 0x3A,
        value: 9,
    },
    PropertyRange {
        start: 0x660,
        end: 0x661,
        value: 0,
    },
    PropertyRange {
        start: 0x661,
        end: 0x662,
        value: 1,
    },
    PropertyRange {
        start: 0x662,
        end: 0x663,
        value: 2,
    },
    PropertyRange {
        start: 0x663,
        end: 0x664,
        value: 3,
    },
    PropertyRange {
        start: 0x664,
        end: 0x665,
        value: 4,
    },
    PropertyRange {
        start: 0x665,
        end: 0x666,
        value: 5,
    },
    PropertyRange {
        start: 0x666,
        end: 0x667,
        value: 6,
    },
    PropertyRange {
        start: 0x667,
        end: 0x668,
        value: 7,
    },
    PropertyRange {
        start: 0x668,
        end: 0x669,
        value: 8,
    },
    PropertyRange {
        start: 0x669,
        end: 0x66A,
        value: 9,
    },
    PropertyRange {
        start: 0x6F0,
        end: 0x6F1,
        value: 0,
    },
    PropertyRange {
        start: 0x6F1,
        end: 0x6F2,
        value: 1,
    },
    PropertyRange {
        start: 0x6F2,
        end: 0x6F3,
        value: 2,
    },
    PropertyRange {
        start: 0x6F3,
        end: 0x6F4,
        value: 3,
    },
    PropertyRange {
        start: 0x6F4,
        end: 0x6F5,
        value: 4,
    },
    PropertyRange {
        start: 0x6F5,
        end: 0x6F6,
        value: 5,
    },
    PropertyRange {
        start: 0x6F6,
        end: 0x6F7,
        value: 6,
    },
    PropertyRange {
        start: 0x6F7,
        end: 0x6F8,
        value: 7,
    },
    PropertyRange {
        start: 0x6F8,
        end: 0x6F9,
        value: 8,
    },
    PropertyRange {
        start: 0x6F9,
        end: 0x6FA,
        value: 9,
    },
    PropertyRange {
        start: 0x7C0,
        end: 0x7C1,
        value: 0,
    },
    PropertyRange {
        start: 0x7C1,
        end: 0x7C2,
        value: 1,
    },
    PropertyRange {
        start: 0x7C2,
        end: 0x7C3,
        value: 2,
    },
    PropertyRange {
        start: 0x7C3,
        end: 0x7C4,
        value: 3,
    },
    PropertyRange {
        start: 0x7C4,
        end: 0x7C5,
        value: 4,
    },
    PropertyRange {
        start: 0x7C5,
        end: 0x7C6,
        value: 5,
    },
    PropertyRange {
        start: 0x7C6,
        end: 0x7C7,
        value: 6,
    },
    PropertyRange {
        start: 0x7C7,
        end: 0x7C8,
        value: 7,
    },
    PropertyRange {
        start: 0x7C8,
        end: 0x7C9,
        value: 8,
    },
    PropertyRange {
        start: 0x7C9,
        end: 0x7CA,
        value: 9,
    },
    PropertyRange {
        start: 0x966,
        end: 0x967,
        value: 0,
    },
    PropertyRange {
        start: 0x967,
        end: 0x968,
        value: 1,
    },
    PropertyRange {
        start: 0x968,
        end: 0x969,
        value: 2,
    },
    PropertyRange {
        start: 0x969,
        end: 0x96A,
        value: 3,
    },
    PropertyRange {
        start: 0x96A,
        end: 0x96B,
        value: 4,
    },
    PropertyRange {
        start: 0x96B,
        end: 0x96C,
        value: 5,
    },
    PropertyRange {
        start: 0x96C,
        end: 0x96D,
        value: 6,
    },
    PropertyRange {
        start: 0x96D,
        end: 0x96E,
        value: 7,
    },
    PropertyRange {
        start: 0x96E,
        end: 0x96F,
        value: 8,
    },
    PropertyRange {
        start: 0x96F,
        end: 0x970,
        value: 9,
    },
    PropertyRange {
        start: 0x9E6,
        end: 0x9E7,
        value: 0,
    },
    PropertyRange {
        start: 0x9E7,
        end: 0x9E8,
        value: 1,
    },
    PropertyRange {
        start: 0x9E8,
        end: 0x9E9,
        value: 2,
    },
    PropertyRange {
        start: 0x9E9,
        end: 0x9EA,
        value: 3,
    },
    PropertyRange {
        start: 0x9EA,
        end: 0x9EB,
        value: 4,
    },
    PropertyRange {
        start: 0x9EB,
        end: 0x9EC,
        value: 5,
    },
    PropertyRange {
        start: 0x9EC,
        end: 0x9ED,
        value: 6,
    },
    PropertyRange {
        start: 0x9ED,
        end: 0x9EE,
        value: 7,
    },
    PropertyRange {
        start: 0x9EE,
        end: 0x9EF,
        value: 8,
    },
    PropertyRange {
        start: 0x9EF,
        end: 0x9F0,
        value: 9,
    },
    PropertyRange {
        start: 0xA66,
        end: 0xA67,
        value: 0,
    },
    PropertyRange {
        start: 0xA67,
        end: 0xA68,
        value: 1,
    },
    PropertyRange {
        start: 0xA68,
        end: 0xA69,
        value: 2,
    },
    PropertyRange {
        start: 0xA69,
        end: 0xA6A,
        value: 3,
    },
    PropertyRange {
        start: 0xA6A,
        end: 0xA6B,
        value: 4,
    },
    PropertyRange {
        start: 0xA6B,
        end: 0xA6C,
        value: 5,
    },
    PropertyRange {
        start: 0xA6C,
        end: 0xA6D,
        value: 6,
    },
    PropertyRange {
        start: 0xA6D,
        end: 0xA6E,
        value: 7,
    },
    PropertyRange {
        start: 0xA6E,
        end: 0xA6F,
        value: 8,
    },
    PropertyRange {
        start: 0xA6F,
        end: 0xA70,
        value: 9,
    },
    PropertyRange {
        start: 0xAE6,
        end: 0xAE7,
        value: 0,
    },
    PropertyRange {
        start: 0xAE7,
        end: 0xAE8,
        value: 1,
    },
    PropertyRange {
        start: 0xAE8,
        end: 0xAE9,
        value: 2,
    },
    PropertyRange {
        start: 0xAE9,
        end: 0xAEA,
        value: 3,
    },
    PropertyRange {
        start: 0xAEA,
        end: 0xAEB,
        value: 4,
    },
    PropertyRange {
        start: 0xAEB,
        end: 0xAEC,
        value: 5,
    },
    PropertyRange {
        start: 0xAEC,
        end: 0xAED,
        value: 6,
    },
    PropertyRange {
        start: 0xAED,
        end: 0xAEE,
        value: 7,
    },
    PropertyRange {
        start: 0xAEE,
        end: 0xAEF,
        value: 8,
    },
    PropertyRange {
        start: 0xAEF,
        end: 0xAF0,
        value: 9,
    },
    PropertyRange {
        start: 0xB66,
        end: 0xB67,
        value: 0,
    },
    PropertyRange {
        start: 0xB67,
        end: 0xB68,
        value: 1,
    },
    PropertyRange {
        start: 0xB68,
        end: 0xB69,
        value: 2,
    },
    PropertyRange {
        start: 0xB69,
        end: 0xB6A,
        value: 3,
    },
    PropertyRange {
        start: 0xB6A,
        end: 0xB6B,
        value: 4,
    },
    PropertyRange {
        start: 0xB6B,
        end: 0xB6C,
        value: 5,
    },
    PropertyRange {
        start: 0xB6C,
        end: 0xB6D,
        value: 6,
    },
    PropertyRange {
        start: 0xB6D,
        end: 0xB6E,
        value: 7,
    },
    PropertyRange {
        start: 0xB6E,
        end: 0xB6F,
        value: 8,
    },
    PropertyRange {
        start: 0xB6F,
        end: 0xB70,
        value: 9,
    },
    PropertyRange {
        start: 0xBE6,
        end: 0xBE7,
        value: 0,
    },
    PropertyRange {
        start: 0xBE7,
        end: 0xBE8,
        value: 1,
    },
    PropertyRange {
        start: 0xBE8,
        end: 0xBE9,
        value: 2,
    },
    PropertyRange {
        start: 0xBE9,
        end: 0xBEA,
        value: 3,
    },
    PropertyRange {
        start: 0xBEA,
        end: 0xBEB,
        value: 4,
    },
    PropertyRange {
        start: 0xBEB,
        end: 0xBEC,
        value: 5,
    },
    PropertyRange {
        start: 0xBEC,
        end: 0xBED,
        value: 6,
    },
    PropertyRange {
        start: 0xBED,
        end: 0xBEE,
        value: 7,
    },
    PropertyRange {
        start: 0xBEE,
        end: 0xBEF,
        value: 8,
    },
    PropertyRange {
        start: 0xBEF,
        end: 0xBF0,
        value: 9,
    },
    PropertyRange {
        start: 0xC66,
        end: 0xC67,
        value: 0,
    },
    PropertyRange {
        start: 0xC67,
        end: 0xC68,
        value: 1,
    },
    PropertyRange {
        start: 0xC68,
        end: 0xC69,
        value: 2,
    },
    PropertyRange {
        start: 0xC69,
        end: 0xC6A,
        value: 3,
    },
    PropertyRange {
        start: 0xC6A,
        end: 0xC6B,
        value: 4,
    },
    PropertyRange {
        start: 0xC6B,
        end: 0xC6C,
        value: 5,
    },
    PropertyRange {
        start: 0xC6C,
        end: 0xC6D,
        value: 6,
    },
    PropertyRange {
        start: 0xC6D,
        end: 0xC6E,
        value: 7,
    },
    PropertyRange {
        start: 0xC6E,
        end: 0xC6F,
        value: 8,
    },
    PropertyRange {
        start: 0xC6F,
        end: 0xC70,
        value: 9,
    },
    PropertyRange {
        start: 0xCE6,
        end: 0xCE7,
        value: 0,
    },
    PropertyRange {
        start: 0xCE7,
        end: 0xCE8,
        value: 1,
    },
    PropertyRange {
        start: 0xCE8,
        end: 0xCE9,
        value: 2,
    },
    PropertyRange {
        start: 0xCE9,
        end: 0xCEA,
        value: 3,
    },
    PropertyRange {
        start: 0xCEA,
        end: 0xCEB,
        value: 4,
    },
    PropertyRange {
        start: 0xCEB,
        end: 0xCEC,
        value: 5,
    },
    PropertyRange {
        start: 0xCEC,
        end: 0xCED,
        value: 6,
    },
    PropertyRange {
        start: 0xCED,
        end: 0xCEE,
        value: 7,
    },
    PropertyRange {
        start: 0xCEE,
        end: 0xCEF,
        value: 8,
    },
    PropertyRange {
        start: 0xCEF,
        end: 0xCF0,
        value: 9,
    },
    PropertyRange {
        start: 0xD66,
        end: 0xD67,
        value: 0,
    },
    PropertyRange {
        start: 0xD67,
        end: 0xD68,
        value: 1,
    },
    PropertyRange {
        start: 0xD68,
        end: 0xD69,
        value: 2,
    },
    PropertyRange {
        start: 0xD69,
        end: 0xD6A,
        value: 3,
    },
    PropertyRange {
        start: 0xD6A,
        end: 0xD6B,
        value: 4,
    },
    PropertyRange {
        start: 0xD6B,
        end: 0xD6C,
        value: 5,
    },
    PropertyRange {
        start: 0xD6C,
        end: 0xD6D,
        value: 6,
    },
    PropertyRange {
        start: 0xD6D,
        end: 0xD6E,
        value: 7,
    },
    PropertyRange {
        start: 0xD6E,
        end: 0xD6F,
        value: 8,
    },
    PropertyRange {
        start: 0xD6F,
        end: 0xD70,
        value: 9,
    },
    PropertyRange {
        start: 0xDE6,
        end: 0xDE7,
        value: 0,
    },
    PropertyRange {
        start: 0xDE7,
        end: 0xDE8,
        value: 1,
    },
    PropertyRange {
        start: 0xDE8,
        end: 0xDE9,
        value: 2,
    },
    PropertyRange {
        start: 0xDE9,
        end: 0xDEA,
        value: 3,
    },
    PropertyRange {
        start: 0xDEA,
        end: 0xDEB,
        value: 4,
    },
    PropertyRange {
        start: 0xDEB,
        end: 0xDEC,
        value: 5,
    },
    PropertyRange {
        start: 0xDEC,
        end: 0xDED,
        value: 6,
    },
    PropertyRange {
        start: 0xDED,
        end: 0xDEE,
        value: 7,
    },
    PropertyRange {
        start: 0xDEE,
        end: 0xDEF,
        value: 8,
    },
    PropertyRange {
        start: 0xDEF,
        end: 0xDF0,
        value: 9,
    },
    PropertyRange {
        start: 0xE50,
        end: 0xE51,
        value: 0,
    },
    PropertyRange {
        start: 0xE51,
        end: 0xE52,
        value: 1,
    },
    PropertyRange {
        start: 0xE52,
        end: 0xE53,
        value: 2,
    },
    PropertyRange {
        start: 0xE53,
        end: 0xE54,
        value: 3,
    },
    PropertyRange {
        start: 0xE54,
        end: 0xE55,
        value: 4,
    },
    PropertyRange {
        start: 0xE55,
        end: 0xE56,
        value: 5,
    },
    PropertyRange {
        start: 0xE56,
        end: 0xE57,
        value: 6,
    },
    PropertyRange {
        start: 0xE57,
        end: 0xE58,
        value: 7,
    },
    PropertyRange {
        start: 0xE58,
        end: 0xE59,
        value: 8,
    },
    PropertyRange {
        start: 0xE59,
        end: 0xE5A,
        value: 9,
    },
    PropertyRange {
        start: 0xED0,
        end: 0xED1,
        value: 0,
    },
    PropertyRange {
        start: 0xED1,
        end: 0xED2,
        value: 1,
    },
    PropertyRange {
        start: 0xED2,
        end: 0xED3,
        value: 2,
    },
    PropertyRange {
        start: 0xED3,
        end: 0xED4,
        value: 3,
    },
    PropertyRange {
        start: 0xED4,
        end: 0xED5,
        value: 4,
    },
    PropertyRange {
        start: 0xED5,
        end: 0xED6,
        value: 5,
    },
    PropertyRange {
        start: 0xED6,
        end: 0xED7,
        value: 6,
    },
    PropertyRange {
        start: 0xED7,
        end: 0xED8,
        value: 7,
    },
    PropertyRange {
        start: 0xED8,
        end: 0xED9,
        value: 8,
    },
    PropertyRange {
        start: 0xED9,
        end: 0xEDA,
        value: 9,
    },
    PropertyRange {
        start: 0xF20,
        end: 0xF21,
        value: 0,
    },
    PropertyRange {
        start: 0xF21,
        end: 0xF22,
        value: 1,
    },
    PropertyRange {
        start: 0xF22,
        end: 0xF23,
        value: 2,
    },
    PropertyRange {
        start: 0xF23,
        end: 0xF24,
        value: 3,
    },
    PropertyRange {
        start: 0xF24,
        end: 0xF25,
        value: 4,
    },
    PropertyRange {
        start: 0xF25,
        end: 0xF26,
        value: 5,
    },
    PropertyRange {
        start: 0xF26,
        end: 0xF27,
        value: 6,
    },
    PropertyRange {
        start: 0xF27,
        end: 0xF28,
        value: 7,
    },
    PropertyRange {
        start: 0xF28,
        end: 0xF29,
        value: 8,
    },
    PropertyRange {
        start: 0xF29,
        end: 0xF2A,
        value: 9,
    },
    PropertyRange {
        start: 0x1040,
        end: 0x1041,
        value: 0,
    },
    PropertyRange {
        start: 0x1041,
        end: 0x1042,
        value: 1,
    },
    PropertyRange {
        start: 0x1042,
        end: 0x1043,
        value: 2,
    },
    PropertyRange {
        start: 0x1043,
        end: 0x1044,
        value: 3,
    },
    PropertyRange {
        start: 0x1044,
        end: 0x1045,
        value: 4,
    },
    PropertyRange {
        start: 0x1045,
        end: 0x1046,
        value: 5,
    },
    PropertyRange {
        start: 0x1046,
        end: 0x1047,
        value: 6,
    },
    PropertyRange {
        start: 0x1047,
        end: 0x1048,
        value: 7,
    },
    PropertyRange {
        start: 0x1048,
        end: 0x1049,
        value: 8,
    },
    PropertyRange {
        start: 0x1049,
        end: 0x104A,
        value: 9,
    },
    PropertyRange {
        start: 0x1090,
        end: 0x1091,
        value: 0,
    },
    PropertyRange {
        start: 0x1091,
        end: 0x1092,
        value: 1,
    },
    PropertyRange {
        start: 0x1092,
        end: 0x1093,
        value: 2,
    },
    PropertyRange {
        start: 0x1093,
        end: 0x1094,
        value: 3,
    },
    PropertyRange {
        start: 0x1094,
        end: 0x1095,
        value: 4,
    },
    PropertyRange {
        start: 0x1095,
        end: 0x1096,
        value: 5,
    },
    PropertyRange {
        start: 0x1096,
        end: 0x1097,
        value: 6,
    },
    PropertyRange {
        start: 0x1097,
        end: 0x1098,
        value: 7,
    },
    PropertyRange {
        start: 0x1098,
        end: 0x1099,
        value: 8,
    },
    PropertyRange {
        start: 0x1099,
        end: 0x109A,
        value: 9,
    },
    PropertyRange {
        start: 0x17E0,
        end: 0x17E1,
        value: 0,
    },
    PropertyRange {
        start: 0x17E1,
        end: 0x17E2,
        value: 1,
    },
    PropertyRange {
        start: 0x17E2,
        end: 0x17E3,
        value: 2,
    },
    PropertyRange {
        start: 0x17E3,
        end: 0x17E4,
        value: 3,
    },
    PropertyRange {
        start: 0x17E4,
        end: 0x17E5,
        value: 4,
    },
    PropertyRange {
        start: 0x17E5,
        end: 0x17E6,
        value: 5,
    },
    PropertyRange {
        start: 0x17E6,
        end: 0x17E7,
        value: 6,
    },
    PropertyRange {
        start: 0x17E7,
        end: 0x17E8,
        value: 7,
    },
    PropertyRange {
        start: 0x17E8,
        end: 0x17E9,
        value: 8,
    },
    PropertyRange {
        start: 0x17E9,
        end: 0x17EA,
        value: 9,
    },
    PropertyRange {
        start: 0x1810,
        end: 0x1811,
        value: 0,
    },
    PropertyRange {
        start: 0x1811,
        end: 0x1812,
        value: 1,
    },
    PropertyRange {
        start: 0x1812,
        end: 0x1813,
        value: 2,
    },
    PropertyRange {
        start: 0x1813,
        end: 0x1814,
        value: 3,
    },
    PropertyRange {
        start: 0x1814,
        end: 0x1815,
        value: 4,
    },
    PropertyRange {
        start: 0x1815,
        end: 0x1816,
        value: 5,
    },
    PropertyRange {
        start: 0x1816,
        end: 0x1817,
        value: 6,
    },
    PropertyRange {
        start: 0x1817,
        end: 0x1818,
        value: 7,
    },
    PropertyRange {
        start: 0x1818,
        end: 0x1819,
        value: 8,
    },
    PropertyRange {
        start: 0x1819,
        end: 0x181A,
        value: 9,
    },
    PropertyRange {
        start: 0x1946,
        end: 0x1947,
        value: 0,
    },
    PropertyRange {
        start: 0x1947,
        end: 0x1948,
        value: 1,
    },
    PropertyRange {
        start: 0x1948,
        end: 0x1949,
        value: 2,
    },
    PropertyRange {
        start: 0x1949,
        end: 0x194A,
        value: 3,
    },
    PropertyRange {
        start: 0x194A,
        end: 0x194B,
        value: 4,
    },
    PropertyRange {
        start: 0x194B,
        end: 0x194C,
        value: 5,
    },
    PropertyRange {
        start: 0x194C,
        end: 0x194D,
        value: 6,
    },
    PropertyRange {
        start: 0x194D,
        end: 0x194E,
        value: 7,
    },
    PropertyRange {
        start: 0x194E,
        end: 0x194F,
        value: 8,
    },
    PropertyRange {
        start: 0x194F,
        end: 0x1950,
        value: 9,
    },
    PropertyRange {
        start: 0x19D0,
        end: 0x19D1,
        value: 0,
    },
    PropertyRange {
        start: 0x19D1,
        end: 0x19D2,
        value: 1,
    },
    PropertyRange {
        start: 0x19D2,
        end: 0x19D3,
        value: 2,
    },
    PropertyRange {
        start: 0x19D3,
        end: 0x19D4,
        value: 3,
    },
    PropertyRange {
        start: 0x19D4,
        end: 0x19D5,
        value: 4,
    },
    PropertyRange {
        start: 0x19D5,
        end: 0x19D6,
        value: 5,
    },
    PropertyRange {
        start: 0x19D6,
        end: 0x19D7,
        value: 6,
    },
    PropertyRange {
        start: 0x19D7,
        end: 0x19D8,
        value: 7,
    },
    PropertyRange {
        start: 0x19D8,
        end: 0x19D9,
        value: 8,
    },
    PropertyRange {
        start: 0x19D9,
        end: 0x19DA,
        value: 9,
    },
    PropertyRange {
        start: 0x1A80,
        end: 0x1A81,
        value: 0,
    },
    PropertyRange {
        start: 0x1A81,
        end: 0x1A82,
        value: 1,
    },
    PropertyRange {
        start: 0x1A82,
        end: 0x1A83,
        value: 2,
    },
    PropertyRange {
        start: 0x1A83,
        end: 0x1A84,
        value: 3,
    },
    PropertyRange {
        start: 0x1A84,
        end: 0x1A85,
        value: 4,
    },
    PropertyRange {
        start: 0x1A85,
        end: 0x1A86,
        value: 5,
    },
    PropertyRange {
        start: 0x1A86,
        end: 0x1A87,
        value: 6,
    },
    PropertyRange {
        start: 0x1A87,
        end: 0x1A88,
        value: 7,
    },
    PropertyRange {
        start: 0x1A88,
        end: 0x1A89,
        value: 8,
    },
    PropertyRange {
        start: 0x1A89,
        end: 0x1A8A,
        value: 9,
    },
    PropertyRange {
        start: 0x1A90,
        end: 0x1A91,
        value: 0,
    },
    PropertyRange {
        start: 0x1A91,
        end: 0x1A92,
        value: 1,
    },
    PropertyRange {
        start: 0x1A92,
        end: 0x1A93,
        value: 2,
    },
    PropertyRange {
        start: 0x1A93,
        end: 0x1A94,
        value: 3,
    },
    PropertyRange {
        start: 0x1A94,
        end: 0x1A95,
        value: 4,
    },
    PropertyRange {
        start: 0x1A95,
        end: 0x1A96,
        value: 5,
    },
    PropertyRange {
        start: 0x1A96,
        end: 0x1A97,
        value: 6,
    },
    PropertyRange {
        start: 0x1A97,
        end: 0x1A98,
        value: 7,
    },
    PropertyRange {
        start: 0x1A98,
        end: 0x1A99,
        value: 8,
    },
    PropertyRange {
        start: 0x1A99,
        end: 0x1A9A,
        value: 9,
    },
    PropertyRange {
        start: 0x1B50,
        end: 0x1B51,
        value: 0,
    },
    PropertyRange {
        start: 0x1B51,
        end: 0x1B52,
        value: 1,
    },
    PropertyRange {
        start: 0x1B52,
        end: 0x1B53,
        value: 2,
    },
    PropertyRange {
        start: 0x1B53,
        end: 0x1B54,
        value: 3,
    },
    PropertyRange {
        start: 0x1B54,
        end: 0x1B55,
        value: 4,
    },
    PropertyRange {
        start: 0x1B55,
        end: 0x1B56,
        value: 5,
    },
    PropertyRange {
        start: 0x1B56,
        end: 0x1B57,
        value: 6,
    },
    PropertyRange {
        start: 0x1B57,
        end: 0x1B58,
        value: 7,
    },
    PropertyRange {
        start: 0x1B58,
        end: 0x1B59,
        value: 8,
    },
    PropertyRange {
        start: 0x1B59,
        end: 0x1B5A,
        value: 9,
    },
    PropertyRange {
        start: 0x1BB0,
        end: 0x1BB1,
        value: 0,
    },
    PropertyRange {
        start: 0x1BB1,
        end: 0x1BB2,
        value: 1,
    },
    PropertyRange {
        start: 0x1BB2,
        end: 0x1BB3,
        value: 2,
    },
    PropertyRange {
        start: 0x1BB3,
        end: 0x1BB4,
        value: 3,
    },
    PropertyRange {
        start: 0x1BB4,
        end: 0x1BB5,
        value: 4,
    },
    PropertyRange {
        start: 0x1BB5,
        end: 0x1BB6,
        value: 5,
    },
    PropertyRange {
        start: 0x1BB6,
        end: 0x1BB7,
        value: 6,
    },
    PropertyRange {
        start: 0x1BB7,
        end: 0x1BB8,
        value: 7,
    },
    PropertyRange {
        start: 0x1BB8,
        end: 0x1BB9,
        value: 8,
    },
    PropertyRange {
        start: 0x1BB9,
        end: 0x1BBA,
        value: 9,
    },
    PropertyRange {
        start: 0x1C40,
        end: 0x1C41,
        value: 0,
    },
    PropertyRange {
        start: 0x1C41,
        end: 0x1C42,
        value: 1,
    },
    PropertyRange {
        start: 0x1C42,
        end: 0x1C43,
        value: 2,
    },
    PropertyRange {
        start: 0x1C43,
        end: 0x1C44,
        value: 3,
    },
    PropertyRange {
        start: 0x1C44,
        end: 0x1C45,
        value: 4,
    },
    PropertyRange {
        start: 0x1C45,
        end: 0x1C46,
        value: 5,
    },
    PropertyRange {
        start: 0x1C46,
        end: 0x1C47,
        value: 6,
    },
    PropertyRange {
        start: 0x1C47,
        end: 0x1C48,
        value: 7,
    },
    PropertyRange {
        start: 0x1C48,
        end: 0x1C49,
        value: 8,
    },
    PropertyRange {
        start: 0x1C49,
        end: 0x1C4A,
        value: 9,
    },
    PropertyRange {
        start: 0x1C50,
        end: 0x1C51,
        value: 0,
    },
    PropertyRange {
        start: 0x1C51,
        end: 0x1C52,
        value: 1,
    },
    PropertyRange {
        start: 0x1C52,
        end: 0x1C53,
        value: 2,
    },
    PropertyRange {
        start: 0x1C53,
        end: 0x1C54,
        value: 3,
    },
    PropertyRange {
        start: 0x1C54,
        end: 0x1C55,
        value: 4,
    },
    PropertyRange {
        start: 0x1C55,
        end: 0x1C56,
        value: 5,
    },
    PropertyRange {
        start: 0x1C56,
        end: 0x1C57,
        value: 6,
    },
    PropertyRange {
        start: 0x1C57,
        end: 0x1C58,
        value: 7,
    },
    PropertyRange {
        start: 0x1C58,
        end: 0x1C59,
        value: 8,
    },
    PropertyRange {
        start: 0x1C59,
        end: 0x1C5A,
        value: 9,
    },
    PropertyRange {
        start: 0xA620,
        end: 0xA621,
        value: 0,
    },
    PropertyRange {
        start: 0xA621,
        end: 0xA622,
        value: 1,
    },
    PropertyRange {
        start: 0xA622,
        end: 0xA623,
        value: 2,
    },
    PropertyRange {
        start: 0xA623,
        end: 0xA624,
        value: 3,
    },
    PropertyRange {
        start: 0xA624,
        end: 0xA625,
        value: 4,
    },
    PropertyRange {
        start: 0xA625,
        end: 0xA626,
        value: 5,
    },
    PropertyRange {
        start: 0xA626,
        end: 0xA627,
        value: 6,
    },
    PropertyRange {
        start: 0xA627,
        end: 0xA628,
        value: 7,
    },
    PropertyRange {
        start: 0xA628,
        end: 0xA629,
        value: 8,
    },
    PropertyRange {
        start: 0xA629,
        end: 0xA62A,
        value: 9,
    },
    PropertyRange {
        start: 0xA8D0,
        end: 0xA8D1,
        value: 0,
    },
    PropertyRange {
        start: 0xA8D1,
        end: 0xA8D2,
        value: 1,
    },
    PropertyRange {
        start: 0xA8D2,
        end: 0xA8D3,
        value: 2,
    },
    PropertyRange {
        start: 0xA8D3,
        end: 0xA8D4,
        value: 3,
    },
    PropertyRange {
        start: 0xA8D4,
        end: 0xA8D5,
        value: 4,
    },
    PropertyRange {
        start: 0xA8D5,
        end: 0xA8D6,
        value: 5,
    },
    PropertyRange {
        start: 0xA8D6,
        end: 0xA8D7,
        value: 6,
    },
    PropertyRange {
        start: 0xA8D7,
        end: 0xA8D8,
        value: 7,
    },
    PropertyRange {
        start: 0xA8D8,
        end: 0xA8D9,
        value: 8,
    },
    PropertyRange {
        start: 0xA8D9,
        end: 0xA8DA,
        value: 9,
    },
    PropertyRange {
        start: 0xA900,
        end: 0xA901,
        value: 0,
    },
    PropertyRange {
        start: 0xA901,
        end: 0xA902,
        value: 1,
    },
    PropertyRange {
        start: 0xA902,
        end: 0xA903,
        value: 2,
    },
    PropertyRange {
        start: 0xA903,
        end: 0xA904,
        value: 3,
    },
    PropertyRange {
        start: 0xA904,
        end: 0xA905,
        value: 4,
    },
    PropertyRange {
        start: 0xA905,
        end: 0xA906,
        value: 5,
    },
    PropertyRange {
        start: 0xA906,
        end: 0xA907,
        value: 6,
    },
    PropertyRange {
        start: 0xA907,
        end: 0xA908,
        value: 7,
    },
    PropertyRange {
        start: 0xA908,
        end: 0xA909,
        value: 8,
    },
    PropertyRange {
        start: 0xA909,
        end: 0xA90A,
        value: 9,
    },
    PropertyRange {
        start: 0xA9D0,
        end: 0xA9D1,
        value: 0,
    },
    PropertyRange {
        start: 0xA9D1,
        end: 0xA9D2,
        value: 1,
    },
    PropertyRange {
        start: 0xA9D2,
        end: 0xA9D3,
        value: 2,
    },
    PropertyRange {
        start: 0xA9D3,
        end: 0xA9D4,
        value: 3,
    },
    PropertyRange {
        start: 0xA9D4,
        end: 0xA9D5,
        value: 4,
    },
    PropertyRange {
        start: 0xA9D5,
        end: 0xA9D6,
        value: 5,
    },
    PropertyRange {
        start: 0xA9D6,
        end: 0xA9D7,
        value: 6,
    },
    PropertyRange {
        start: 0xA9D7,
        end: 0xA9D8,
        value: 7,
    },
    PropertyRange {
        start: 0xA9D8,
        end: 0xA9D9,
        value: 8,
    },
    PropertyRange {
        start: 0xA9D9,
        end: 0xA9DA,
        value: 9,
    },
    PropertyRange {
        start: 0xA9F0,
        end: 0xA9F1,
        value: 0,
    },
    PropertyRange {
        start: 0xA9F1,
        end: 0xA9F2,
        value: 1,
    },
    PropertyRange {
        start: 0xA9F2,
        end: 0xA9F3,
        value: 2,
    },
    PropertyRange {
        start: 0xA9F3,
        end: 0xA9F4,
        value: 3,
    },
    PropertyRange {
        start: 0xA9F4,
        end: 0xA9F5,
        value: 4,
    },
    PropertyRange {
        start: 0xA9F5,
        end: 0xA9F6,
        value: 5,
    },
    PropertyRange {
        start: 0xA9F6,
        end: 0xA9F7,
        value: 6,
    },
    PropertyRange {
        start: 0xA9F7,
        end: 0xA9F8,
        value: 7,
    },
    PropertyRange {
        start: 0xA9F8,
        end: 0xA9F9,
        value: 8,
    },
    PropertyRange {
        start: 0xA9F9,
        end: 0xA9FA,
        value: 9,
    },
    PropertyRange {
        start: 0xAA50,
        end: 0xAA51,
        value: 0,
    },
    PropertyRange {
        start: 0xAA51,
        end: 0xAA52,
        value: 1,
    },
    PropertyRange {
        start: 0xAA52,
        end: 0xAA53,
        value: 2,
    },
    PropertyRange {
        start: 0xAA53,
        end: 0xAA54,
        value: 3,
    },
    PropertyRange {
        start: 0xAA54,
        end: 0xAA55,
        value: 4,
    },
    PropertyRange {
        start: 0xAA55,
        end: 0xAA56,
        value: 5,
    },
    PropertyRange {
        start: 0xAA56,
        end: 0xAA57,
        value: 6,
    },
    PropertyRange {
        start: 0xAA57,
        end: 0xAA58,
        value: 7,
    },
    PropertyRange {
        start: 0xAA58,
        end: 0xAA59,
        value: 8,
    },
    PropertyRange {
        start: 0xAA59,
        end: 0xAA5A,
        value: 9,
    },
    PropertyRange {
        start: 0xABF0,
        end: 0xABF1,
        value: 0,
    },
    PropertyRange {
        start: 0xABF1,
        end: 0xABF2,
        value: 1,
    },
    PropertyRange {
        start: 0xABF2,
        end: 0xABF3,
        value: 2,
    },
    PropertyRange {
        start: 0xABF3,
        end: 0xABF4,
        value: 3,
    },
    PropertyRange {
        start: 0xABF4,
        end: 0xABF5,
        value: 4,
    },
    PropertyRange {
        start: 0xABF5,
        end: 0xABF6,
        value: 5,
    },
    PropertyRange {
        start: 0xABF6,
        end: 0xABF7,
        value: 6,
    },
    PropertyRange {
        start: 0xABF7,
        end: 0xABF8,
        value: 7,
    },
    PropertyRange {
        start: 0xABF8,
        end: 0xABF9,
        value: 8,
    },
    PropertyRange {
        start: 0xABF9,
        end: 0xABFA,
        value: 9,
    },
    PropertyRange {
        start: 0xFF10,
        end: 0xFF11,
        value: 0,
    },
    PropertyRange {
        start: 0xFF11,
        end: 0xFF12,
        value: 1,
    },
    PropertyRange {
        start: 0xFF12,
        end: 0xFF13,
        value: 2,
    },
    PropertyRange {
        start: 0xFF13,
        end: 0xFF14,
        value: 3,
    },
    PropertyRange {
        start: 0xFF14,
        end: 0xFF15,
        value: 4,
    },
    PropertyRange {
        start: 0xFF15,
        end: 0xFF16,
        value: 5,
    },
    PropertyRange {
        start: 0xFF16,
        end: 0xFF17,
        value: 6,
    },
    PropertyRange {
        start: 0xFF17,
        end: 0xFF18,
        value: 7,
    },
    PropertyRange {
        start: 0xFF18,
        end: 0xFF19,
        value: 8,
    },
    PropertyRange {
        start: 0xFF19,
        end: 0xFF1A,
        value: 9,
    },
    PropertyRange {
        start: 0x104A0,
        end: 0x104A1,
        value: 0,
    },
    PropertyRange {
        start: 0x104A1,
        end: 0x104A2,
        value: 1,
    },
    PropertyRange {
        start: 0x104A2,
        end: 0x104A3,
        value: 2,
    },
    PropertyRange {
        start: 0x104A3,
        end: 0x104A4,
        value: 3,
    },
    PropertyRange {
        start: 0x104A4,
        end: 0x104A5,
        value: 4,
    },
    PropertyRange {
        start: 0x104A5,
        end: 0x104A6,
        value: 5,
    },
    PropertyRange {
        start: 0x104A6,
        end: 0x104A7,
        value: 6,
    },
    PropertyRange {
        start: 0x104A7,
        end: 0x104A8,
        value: 7,
    },
    PropertyRange {
        start: 0x104A8,
        end: 0x104A9,
        value: 8,
    },
    PropertyRange {
        start: 0x104A9,
        end: 0x104AA,
        value: 9,
    },
    PropertyRange {
        start: 0x10D30,
        end: 0x10D31,
        value: 0,
    },
    PropertyRange {
        start: 0x10D31,
        end: 0x10D32,
        value: 1,
    },
    PropertyRange {
        start: 0x10D32,
        end: 0x10D33,
        value: 2,
    },
    PropertyRange {
        start: 0x10D33,
        end: 0x10D34,
        value: 3,
    },
    PropertyRange {
        start: 0x10D34,
        end: 0x10D35,
        value: 4,
    },
    PropertyRange {
        start: 0x10D35,
        end: 0x10D36,
        value: 5,
    },
    PropertyRange {
        start: 0x10D36,
        end: 0x10D37,
        value: 6,
    },
    PropertyRange {
        start: 0x10D37,
        end: 0x10D38,
        value: 7,
    },
    PropertyRange {
        start: 0x10D38,
        end: 0x10D39,
        value: 8,
    },
    PropertyRange {
        start: 0x10D39,
        end: 0x10D3A,
        value: 9,
    },
    PropertyRange {
        start: 0x10D40,
        end: 0x10D41,
        value: 0,
    },
    PropertyRange {
        start: 0x10D41,
        end: 0x10D42,
        value: 1,
    },
    PropertyRange {
        start: 0x10D42,
        end: 0x10D43,
        value: 2,
    },
    PropertyRange {
        start: 0x10D43,
        end: 0x10D44,
        value: 3,
    },
    PropertyRange {
        start: 0x10D44,
        end: 0x10D45,
        value: 4,
    },
    PropertyRange {
        start: 0x10D45,
        end: 0x10D46,
        value: 5,
    },
    PropertyRange {
        start: 0x10D46,
        end: 0x10D47,
        value: 6,
    },
    PropertyRange {
        start: 0x10D47,
        end: 0x10D48,
        value: 7,
    },
    PropertyRange {
        start: 0x10D48,
        end: 0x10D49,
        value: 8,
    },
    PropertyRange {
        start: 0x10D49,
        end: 0x10D4A,
        value: 9,
    },
    PropertyRange {
        start: 0x11066,
        end: 0x11067,
        value: 0,
    },
    PropertyRange {
        start: 0x11067,
        end: 0x11068,
        value: 1,
    },
    PropertyRange {
        start: 0x11068,
        end: 0x11069,
        value: 2,
    },
    PropertyRange {
        start: 0x11069,
        end: 0x1106A,
        value: 3,
    },
    PropertyRange {
        start: 0x1106A,
        end: 0x1106B,
        value: 4,
    },
    PropertyRange {
        start: 0x1106B,
        end: 0x1106C,
        value: 5,
    },
    PropertyRange {
        start: 0x1106C,
        end: 0x1106D,
        value: 6,
    },
    PropertyRange {
        start: 0x1106D,
        end: 0x1106E,
        value: 7,
    },
    PropertyRange {
        start: 0x1106E,
        end: 0x1106F,
        value: 8,
    },
    PropertyRange {
        start: 0x1106F,
        end: 0x11070,
        value: 9,
    },
    PropertyRange {
        start: 0x110F0,
        end: 0x110F1,
        value: 0,
    },
    PropertyRange {
        start: 0x110F1,
        end: 0x110F2,
        value: 1,
    },
    PropertyRange {
        start: 0x110F2,
        end: 0x110F3,
        value: 2,
    },
    PropertyRange {
        start: 0x110F3,
        end: 0x110F4,
        value: 3,
    },
    PropertyRange {
        start: 0x110F4,
        end: 0x110F5,
        value: 4,
    },
    PropertyRange {
        start: 0x110F5,
        end: 0x110F6,
        value: 5,
    },
    PropertyRange {
        start: 0x110F6,
        end: 0x110F7,
        value: 6,
    },
    PropertyRange {
        start: 0x110F7,
        end: 0x110F8,
        value: 7,
    },
    PropertyRange {
        start: 0x110F8,
        end: 0x110F9,
        value: 8,
    },
    PropertyRange {
        start: 0x110F9,
        end: 0x110FA,
        value: 9,
    },
    PropertyRange {
        start: 0x11136,
        end: 0x11137,
        value: 0,
    },
    PropertyRange {
        start: 0x11137,
        end: 0x11138,
        value: 1,
    },
    PropertyRange {
        start: 0x11138,
        end: 0x11139,
        value: 2,
    },
    PropertyRange {
        start: 0x11139,
        end: 0x1113A,
        value: 3,
    },
    PropertyRange {
        start: 0x1113A,
        end: 0x1113B,
        value: 4,
    },
    PropertyRange {
        start: 0x1113B,
        end: 0x1113C,
        value: 5,
    },
    PropertyRange {
        start: 0x1113C,
        end: 0x1113D,
        value: 6,
    },
    PropertyRange {
        start: 0x1113D,
        end: 0x1113E,
        value: 7,
    },
    PropertyRange {
        start: 0x1113E,
        end: 0x1113F,
        value: 8,
    },
    PropertyRange {
        start: 0x1113F,
        end: 0x11140,
        value: 9,
    },
    PropertyRange {
        start: 0x111D0,
        end: 0x111D1,
        value: 0,
    },
    PropertyRange {
        start: 0x111D1,
        end: 0x111D2,
        value: 1,
    },
    PropertyRange {
        start: 0x111D2,
        end: 0x111D3,
        value: 2,
    },
    PropertyRange {
        start: 0x111D3,
        end: 0x111D4,
        value: 3,
    },
    PropertyRange {
        start: 0x111D4,
        end: 0x111D5,
        value: 4,
    },
    PropertyRange {
        start: 0x111D5,
        end: 0x111D6,
        value: 5,
    },
    PropertyRange {
        start: 0x111D6,
        end: 0x111D7,
        value: 6,
    },
    PropertyRange {
        start: 0x111D7,
        end: 0x111D8,
        value: 7,
    },
    PropertyRange {
        start: 0x111D8,
        end: 0x111D9,
        value: 8,
    },
    PropertyRange {
        start: 0x111D9,
        end: 0x111DA,
        value: 9,
    },
    PropertyRange {
        start: 0x112F0,
        end: 0x112F1,
        value: 0,
    },
    PropertyRange {
        start: 0x112F1,
        end: 0x112F2,
        value: 1,
    },
    PropertyRange {
        start: 0x112F2,
        end: 0x112F3,
        value: 2,
    },
    PropertyRange {
        start: 0x112F3,
        end: 0x112F4,
        value: 3,
    },
    PropertyRange {
        start: 0x112F4,
        end: 0x112F5,
        value: 4,
    },
    PropertyRange {
        start: 0x112F5,
        end: 0x112F6,
        value: 5,
    },
    PropertyRange {
        start: 0x112F6,
        end: 0x112F7,
        value: 6,
    },
    PropertyRange {
        start: 0x112F7,
        end: 0x112F8,
        value: 7,
    },
    PropertyRange {
        start: 0x112F8,
        end: 0x112F9,
        value: 8,
    },
    PropertyRange {
        start: 0x112F9,
        end: 0x112FA,
        value: 9,
    },
    PropertyRange {
        start: 0x11450,
        end: 0x11451,
        value: 0,
    },
    PropertyRange {
        start: 0x11451,
        end: 0x11452,
        value: 1,
    },
    PropertyRange {
        start: 0x11452,
        end: 0x11453,
        value: 2,
    },
    PropertyRange {
        start: 0x11453,
        end: 0x11454,
        value: 3,
    },
    PropertyRange {
        start: 0x11454,
        end: 0x11455,
        value: 4,
    },
    PropertyRange {
        start: 0x11455,
        end: 0x11456,
        value: 5,
    },
    PropertyRange {
        start: 0x11456,
        end: 0x11457,
        value: 6,
    },
    PropertyRange {
        start: 0x11457,
        end: 0x11458,
        value: 7,
    },
    PropertyRange {
        start: 0x11458,
        end: 0x11459,
        value: 8,
    },
    PropertyRange {
        start: 0x11459,
        end: 0x1145A,
        value: 9,
    },
    PropertyRange {
        start: 0x114D0,
        end: 0x114D1,
        value: 0,
    },
    PropertyRange {
        start: 0x114D1,
        end: 0x114D2,
        value: 1,
    },
    PropertyRange {
        start: 0x114D2,
        end: 0x114D3,
        value: 2,
    },
    PropertyRange {
        start: 0x114D3,
        end: 0x114D4,
        value: 3,
    },
    PropertyRange {
        start: 0x114D4,
        end: 0x114D5,
        value: 4,
    },
    PropertyRange {
        start: 0x114D5,
        end: 0x114D6,
        value: 5,
    },
    PropertyRange {
        start: 0x114D6,
        end: 0x114D7,
        value: 6,
    },
    PropertyRange {
        start: 0x114D7,
        end: 0x114D8,
        value: 7,
    },
    PropertyRange {
        start: 0x114D8,
        end: 0x114D9,
        value: 8,
    },
    PropertyRange {
        start: 0x114D9,
        end: 0x114DA,
        value: 9,
    },
    PropertyRange {
        start: 0x11650,
        end: 0x11651,
        value: 0,
    },
    PropertyRange {
        start: 0x11651,
        end: 0x11652,
        value: 1,
    },
    PropertyRange {
        start: 0x11652,
        end: 0x11653,
        value: 2,
    },
    PropertyRange {
        start: 0x11653,
        end: 0x11654,
        value: 3,
    },
    PropertyRange {
        start: 0x11654,
        end: 0x11655,
        value: 4,
    },
    PropertyRange {
        start: 0x11655,
        end: 0x11656,
        value: 5,
    },
    PropertyRange {
        start: 0x11656,
        end: 0x11657,
        value: 6,
    },
    PropertyRange {
        start: 0x11657,
        end: 0x11658,
        value: 7,
    },
    PropertyRange {
        start: 0x11658,
        end: 0x11659,
        value: 8,
    },
    PropertyRange {
        start: 0x11659,
        end: 0x1165A,
        value: 9,
    },
    PropertyRange {
        start: 0x116C0,
        end: 0x116C1,
        value: 0,
    },
    PropertyRange {
        start: 0x116C1,
        end: 0x116C2,
        value: 1,
    },
    PropertyRange {
        start: 0x116C2,
        end: 0x116C3,
        value: 2,
    },
    PropertyRange {
        start: 0x116C3,
        end: 0x116C4,
        value: 3,
    },
    PropertyRange {
        start: 0x116C4,
        end: 0x116C5,
        value: 4,
    },
    PropertyRange {
        start: 0x116C5,
        end: 0x116C6,
        value: 5,
    },
    PropertyRange {
        start: 0x116C6,
        end: 0x116C7,
        value: 6,
    },
    PropertyRange {
        start: 0x116C7,
        end: 0x116C8,
        value: 7,
    },
    PropertyRange {
        start: 0x116C8,
        end: 0x116C9,
        value: 8,
    },
    PropertyRange {
        start: 0x116C9,
        end: 0x116CA,
        value: 9,
    },
    PropertyRange {
        start: 0x116D0,
        end: 0x116D1,
        value: 0,
    },
    PropertyRange {
        start: 0x116D1,
        end: 0x116D2,
        value: 1,
    },
    PropertyRange {
        start: 0x116D2,
        end: 0x116D3,
        value: 2,
    },
    PropertyRange {
        start: 0x116D3,
        end: 0x116D4,
        value: 3,
    },
    PropertyRange {
        start: 0x116D4,
        end: 0x116D5,
        value: 4,
    },
    PropertyRange {
        start: 0x116D5,
        end: 0x116D6,
        value: 5,
    },
    PropertyRange {
        start: 0x116D6,
        end: 0x116D7,
        value: 6,
    },
    PropertyRange {
        start: 0x116D7,
        end: 0x116D8,
        value: 7,
    },
    PropertyRange {
        start: 0x116D8,
        end: 0x116D9,
        value: 8,
    },
    PropertyRange {
        start: 0x116D9,
        end: 0x116DA,
        value: 9,
    },
    PropertyRange {
        start: 0x116DA,
        end: 0x116DB,
        value: 0,
    },
    PropertyRange {
        start: 0x116DB,
        end: 0x116DC,
        value: 1,
    },
    PropertyRange {
        start: 0x116DC,
        end: 0x116DD,
        value: 2,
    },
    PropertyRange {
        start: 0x116DD,
        end: 0x116DE,
        value: 3,
    },
    PropertyRange {
        start: 0x116DE,
        end: 0x116DF,
        value: 4,
    },
    PropertyRange {
        start: 0x116DF,
        end: 0x116E0,
        value: 5,
    },
    PropertyRange {
        start: 0x116E0,
        end: 0x116E1,
        value: 6,
    },
    PropertyRange {
        start: 0x116E1,
        end: 0x116E2,
        value: 7,
    },
    PropertyRange {
        start: 0x116E2,
        end: 0x116E3,
        value: 8,
    },
    PropertyRange {
        start: 0x116E3,
        end: 0x116E4,
        value: 9,
    },
    PropertyRange {
        start: 0x11730,
        end: 0x11731,
        value: 0,
    },
    PropertyRange {
        start: 0x11731,
        end: 0x11732,
        value: 1,
    },
    PropertyRange {
        start: 0x11732,
        end: 0x11733,
        value: 2,
    },
    PropertyRange {
        start: 0x11733,
        end: 0x11734,
        value: 3,
    },
    PropertyRange {
        start: 0x11734,
        end: 0x11735,
        value: 4,
    },
    PropertyRange {
        start: 0x11735,
        end: 0x11736,
        value: 5,
    },
    PropertyRange {
        start: 0x11736,
        end: 0x11737,
        value: 6,
    },
    PropertyRange {
        start: 0x11737,
        end: 0x11738,
        value: 7,
    },
    PropertyRange {
        start: 0x11738,
        end: 0x11739,
        value: 8,
    },
    PropertyRange {
        start: 0x11739,
        end: 0x1173A,
        value: 9,
    },
    PropertyRange {
        start: 0x118E0,
        end: 0x118E1,
        value: 0,
    },
    PropertyRange {
        start: 0x118E1,
        end: 0x118E2,
        value: 1,
    },
    PropertyRange {
        start: 0x118E2,
        end: 0x118E3,
        value: 2,
    },
    PropertyRange {
        start: 0x118E3,
        end: 0x118E4,
        value: 3,
    },
    PropertyRange {
        start: 0x118E4,
        end: 0x118E5,
        value: 4,
    },
    PropertyRange {
        start: 0x118E5,
        end: 0x118E6,
        value: 5,
    },
    PropertyRange {
        start: 0x118E6,
        end: 0x118E7,
        value: 6,
    },
    PropertyRange {
        start: 0x118E7,
        end: 0x118E8,
        value: 7,
    },
    PropertyRange {
        start: 0x118E8,
        end: 0x118E9,
        value: 8,
    },
    PropertyRange {
        start: 0x118E9,
        end: 0x118EA,
        value: 9,
    },
    PropertyRange {
        start: 0x11950,
        end: 0x11951,
        value: 0,
    },
    PropertyRange {
        start: 0x11951,
        end: 0x11952,
        value: 1,
    },
    PropertyRange {
        start: 0x11952,
        end: 0x11953,
        value: 2,
    },
    PropertyRange {
        start: 0x11953,
        end: 0x11954,
        value: 3,
    },
    PropertyRange {
        start: 0x11954,
        end: 0x11955,
        value: 4,
    },
    PropertyRange {
        start: 0x11955,
        end: 0x11956,
        value: 5,
    },
    PropertyRange {
        start: 0x11956,
        end: 0x11957,
        value: 6,
    },
    PropertyRange {
        start: 0x11957,
        end: 0x11958,
        value: 7,
    },
    PropertyRange {
        start: 0x11958,
        end: 0x11959,
        value: 8,
    },
    PropertyRange {
        start: 0x11959,
        end: 0x1195A,
        value: 9,
    },
    PropertyRange {
        start: 0x11BF0,
        end: 0x11BF1,
        value: 0,
    },
    PropertyRange {
        start: 0x11BF1,
        end: 0x11BF2,
        value: 1,
    },
    PropertyRange {
        start: 0x11BF2,
        end: 0x11BF3,
        value: 2,
    },
    PropertyRange {
        start: 0x11BF3,
        end: 0x11BF4,
        value: 3,
    },
    PropertyRange {
        start: 0x11BF4,
        end: 0x11BF5,
        value: 4,
    },
    PropertyRange {
        start: 0x11BF5,
        end: 0x11BF6,
        value: 5,
    },
    PropertyRange {
        start: 0x11BF6,
        end: 0x11BF7,
        value: 6,
    },
    PropertyRange {
        start: 0x11BF7,
        end: 0x11BF8,
        value: 7,
    },
    PropertyRange {
        start: 0x11BF8,
        end: 0x11BF9,
        value: 8,
    },
    PropertyRange {
        start: 0x11BF9,
        end: 0x11BFA,
        value: 9,
    },
    PropertyRange {
        start: 0x11C50,
        end: 0x11C51,
        value: 0,
    },
    PropertyRange {
        start: 0x11C51,
        end: 0x11C52,
        value: 1,
    },
    PropertyRange {
        start: 0x11C52,
        end: 0x11C53,
        value: 2,
    },
    PropertyRange {
        start: 0x11C53,
        end: 0x11C54,
        value: 3,
    },
    PropertyRange {
        start: 0x11C54,
        end: 0x11C55,
        value: 4,
    },
    PropertyRange {
        start: 0x11C55,
        end: 0x11C56,
        value: 5,
    },
    PropertyRange {
        start: 0x11C56,
        end: 0x11C57,
        value: 6,
    },
    PropertyRange {
        start: 0x11C57,
        end: 0x11C58,
        value: 7,
    },
    PropertyRange {
        start: 0x11C58,
        end: 0x11C59,
        value: 8,
    },
    PropertyRange {
        start: 0x11C59,
        end: 0x11C5A,
        value: 9,
    },
    PropertyRange {
        start: 0x11D50,
        end: 0x11D51,
        value: 0,
    },
    PropertyRange {
        start: 0x11D51,
        end: 0x11D52,
        value: 1,
    },
    PropertyRange {
        start: 0x11D52,
        end: 0x11D53,
        value: 2,
    },
    PropertyRange {
        start: 0x11D53,
        end: 0x11D54,
        value: 3,
    },
    PropertyRange {
        start: 0x11D54,
        end: 0x11D55,
        value: 4,
    },
    PropertyRange {
        start: 0x11D55,
        end: 0x11D56,
        value: 5,
    },
    PropertyRange {
        start: 0x11D56,
        end: 0x11D57,
        value: 6,
    },
    PropertyRange {
        start: 0x11D57,
        end: 0x11D58,
        value: 7,
    },
    PropertyRange {
        start: 0x11D58,
        end: 0x11D59,
        value: 8,
    },
    PropertyRange {
        start: 0x11D59,
        end: 0x11D5A,
        value: 9,
    },
    PropertyRange {
        start: 0x11DA0,
        end: 0x11DA1,
        value: 0,
    },
    PropertyRange {
        start: 0x11DA1,
        end: 0x11DA2,
        value: 1,
    },
    PropertyRange {
        start: 0x11DA2,
        end: 0x11DA3,
        value: 2,
    },
    PropertyRange {
        start: 0x11DA3,
        end: 0x11DA4,
        value: 3,
    },
    PropertyRange {
        start: 0x11DA4,
        end: 0x11DA5,
        value: 4,
    },
    PropertyRange {
        start: 0x11DA5,
        end: 0x11DA6,
        value: 5,
    },
    PropertyRange {
        start: 0x11DA6,
        end: 0x11DA7,
        value: 6,
    },
    PropertyRange {
        start: 0x11DA7,
        end: 0x11DA8,
        value: 7,
    },
    PropertyRange {
        start: 0x11DA8,
        end: 0x11DA9,
        value: 8,
    },
    PropertyRange {
        start: 0x11DA9,
        end: 0x11DAA,
        value: 9,
    },
    PropertyRange {
        start: 0x11F50,
        end: 0x11F51,
        value: 0,
    },
    PropertyRange {
        start: 0x11F51,
        end: 0x11F52,
        value: 1,
    },
    PropertyRange {
        start: 0x11F52,
        end: 0x11F53,
        value: 2,
    },
    PropertyRange {
        start: 0x11F53,
        end: 0x11F54,
        value: 3,
    },
    PropertyRange {
        start: 0x11F54,
        end: 0x11F55,
        value: 4,
    },
    PropertyRange {
        start: 0x11F55,
        end: 0x11F56,
        value: 5,
    },
    PropertyRange {
        start: 0x11F56,
        end: 0x11F57,
        value: 6,
    },
    PropertyRange {
        start: 0x11F57,
        end: 0x11F58,
        value: 7,
    },
    PropertyRange {
        start: 0x11F58,
        end: 0x11F59,
        value: 8,
    },
    PropertyRange {
        start: 0x11F59,
        end: 0x11F5A,
        value: 9,
    },
    PropertyRange {
        start: 0x16130,
        end: 0x16131,
        value: 0,
    },
    PropertyRange {
        start: 0x16131,
        end: 0x16132,
        value: 1,
    },
    PropertyRange {
        start: 0x16132,
        end: 0x16133,
        value: 2,
    },
    PropertyRange {
        start: 0x16133,
        end: 0x16134,
        value: 3,
    },
    PropertyRange {
        start: 0x16134,
        end: 0x16135,
        value: 4,
    },
    PropertyRange {
        start: 0x16135,
        end: 0x16136,
        value: 5,
    },
    PropertyRange {
        start: 0x16136,
        end: 0x16137,
        value: 6,
    },
    PropertyRange {
        start: 0x16137,
        end: 0x16138,
        value: 7,
    },
    PropertyRange {
        start: 0x16138,
        end: 0x16139,
        value: 8,
    },
    PropertyRange {
        start: 0x16139,
        end: 0x1613A,
        value: 9,
    },
    PropertyRange {
        start: 0x16A60,
        end: 0x16A61,
        value: 0,
    },
    PropertyRange {
        start: 0x16A61,
        end: 0x16A62,
        value: 1,
    },
    PropertyRange {
        start: 0x16A62,
        end: 0x16A63,
        value: 2,
    },
    PropertyRange {
        start: 0x16A63,
        end: 0x16A64,
        value: 3,
    },
    PropertyRange {
        start: 0x16A64,
        end: 0x16A65,
        value: 4,
    },
    PropertyRange {
        start: 0x16A65,
        end: 0x16A66,
        value: 5,
    },
    PropertyRange {
        start: 0x16A66,
        end: 0x16A67,
        value: 6,
    },
    PropertyRange {
        start: 0x16A67,
        end: 0x16A68,
        value: 7,
    },
    PropertyRange {
        start: 0x16A68,
        end: 0x16A69,
        value: 8,
    },
    PropertyRange {
        start: 0x16A69,
        end: 0x16A6A,
        value: 9,
    },
    PropertyRange {
        start: 0x16AC0,
        end: 0x16AC1,
        value: 0,
    },
    PropertyRange {
        start: 0x16AC1,
        end: 0x16AC2,
        value: 1,
    },
    PropertyRange {
        start: 0x16AC2,
        end: 0x16AC3,
        value: 2,
    },
    PropertyRange {
        start: 0x16AC3,
        end: 0x16AC4,
        value: 3,
    },
    PropertyRange {
        start: 0x16AC4,
        end: 0x16AC5,
        value: 4,
    },
    PropertyRange {
        start: 0x16AC5,
        end: 0x16AC6,
        value: 5,
    },
    PropertyRange {
        start: 0x16AC6,
        end: 0x16AC7,
        value: 6,
    },
    PropertyRange {
        start: 0x16AC7,
        end: 0x16AC8,
        value: 7,
    },
    PropertyRange {
        start: 0x16AC8,
        end: 0x16AC9,
        value: 8,
    },
    PropertyRange {
        start: 0x16AC9,
        end: 0x16ACA,
        value: 9,
    },
    PropertyRange {
        start: 0x16B50,
        end: 0x16B51,
        value: 0,
    },
    PropertyRange {
        start: 0x16B51,
        end: 0x16B52,
        value: 1,
    },
    PropertyRange {
        start: 0x16B52,
        end: 0x16B53,
        value: 2,
    },
    PropertyRange {
        start: 0x16B53,
        end: 0x16B54,
        value: 3,
    },
    PropertyRange {
        start: 0x16B54,
        end: 0x16B55,
        value: 4,
    },
    PropertyRange {
        start: 0x16B55,
        end: 0x16B56,
        value: 5,
    },
    PropertyRange {
        start: 0x16B56,
        end: 0x16B57,
        value: 6,
    },
    PropertyRange {
        start: 0x16B57,
        end: 0x16B58,
        value: 7,
    },
    PropertyRange {
        start: 0x16B58,
        end: 0x16B59,
        value: 8,
    },
    PropertyRange {
        start: 0x16B59,
        end: 0x16B5A,
        value: 9,
    },
    PropertyRange {
        start: 0x16D70,
        end: 0x16D71,
        value: 0,
    },
    PropertyRange {
        start: 0x16D71,
        end: 0x16D72,
        value: 1,
    },
    PropertyRange {
        start: 0x16D72,
        end: 0x16D73,
        value: 2,
    },
    PropertyRange {
        start: 0x16D73,
        end: 0x16D74,
        value: 3,
    },
    PropertyRange {
        start: 0x16D74,
        end: 0x16D75,
        value: 4,
    },
    PropertyRange {
        start: 0x16D75,
        end: 0x16D76,
        value: 5,
    },
    PropertyRange {
        start: 0x16D76,
        end: 0x16D77,
        value: 6,
    },
    PropertyRange {
        start: 0x16D77,
        end: 0x16D78,
        value: 7,
    },
    PropertyRange {
        start: 0x16D78,
        end: 0x16D79,
        value: 8,
    },
    PropertyRange {
        start: 0x16D79,
        end: 0x16D7A,
        value: 9,
    },
    PropertyRange {
        start: 0x1CCF0,
        end: 0x1CCF1,
        value: 0,
    },
    PropertyRange {
        start: 0x1CCF1,
        end: 0x1CCF2,
        value: 1,
    },
    PropertyRange {
        start: 0x1CCF2,
        end: 0x1CCF3,
        value: 2,
    },
    PropertyRange {
        start: 0x1CCF3,
        end: 0x1CCF4,
        value: 3,
    },
    PropertyRange {
        start: 0x1CCF4,
        end: 0x1CCF5,
        value: 4,
    },
    PropertyRange {
        start: 0x1CCF5,
        end: 0x1CCF6,
        value: 5,
    },
    PropertyRange {
        start: 0x1CCF6,
        end: 0x1CCF7,
        value: 6,
    },
    PropertyRange {
        start: 0x1CCF7,
        end: 0x1CCF8,
        value: 7,
    },
    PropertyRange {
        start: 0x1CCF8,
        end: 0x1CCF9,
        value: 8,
    },
    PropertyRange {
        start: 0x1CCF9,
        end: 0x1CCFA,
        value: 9,
    },
    PropertyRange {
        start: 0x1D7CE,
        end: 0x1D7CF,
        value: 0,
    },
    PropertyRange {
        start: 0x1D7CF,
        end: 0x1D7D0,
        value: 1,
    },
    PropertyRange {
        start: 0x1D7D0,
        end: 0x1D7D1,
        value: 2,
    },
    PropertyRange {
        start: 0x1D7D1,
        end: 0x1D7D2,
        value: 3,
    },
    PropertyRange {
        start: 0x1D7D2,
        end: 0x1D7D3,
        value: 4,
    },
    PropertyRange {
        start: 0x1D7D3,
        end: 0x1D7D4,
        value: 5,
    },
    PropertyRange {
        start: 0x1D7D4,
        end: 0x1D7D5,
        value: 6,
    },
    PropertyRange {
        start: 0x1D7D5,
        end: 0x1D7D6,
        value: 7,
    },
    PropertyRange {
        start: 0x1D7D6,
        end: 0x1D7D7,
        value: 8,
    },
    PropertyRange {
        start: 0x1D7D7,
        end: 0x1D7D8,
        value: 9,
    },
    PropertyRange {
        start: 0x1D7D8,
        end: 0x1D7D9,
        value: 0,
    },
    PropertyRange {
        start: 0x1D7D9,
        end: 0x1D7DA,
        value: 1,
    },
    PropertyRange {
        start: 0x1D7DA,
        end: 0x1D7DB,
        value: 2,
    },
    PropertyRange {
        start: 0x1D7DB,
        end: 0x1D7DC,
        value: 3,
    },
    PropertyRange {
        start: 0x1D7DC,
        end: 0x1D7DD,
        value: 4,
    },
    PropertyRange {
        start: 0x1D7DD,
        end: 0x1D7DE,
        value: 5,
    },
    PropertyRange {
        start: 0x1D7DE,
        end: 0x1D7DF,
        value: 6,
    },
    PropertyRange {
        start: 0x1D7DF,
        end: 0x1D7E0,
        value: 7,
    },
    PropertyRange {
        start: 0x1D7E0,
        end: 0x1D7E1,
        value: 8,
    },
    PropertyRange {
        start: 0x1D7E1,
        end: 0x1D7E2,
        value: 9,
    },
    PropertyRange {
        start: 0x1D7E2,
        end: 0x1D7E3,
        value: 0,
    },
    PropertyRange {
        start: 0x1D7E3,
        end: 0x1D7E4,
        value: 1,
    },
    PropertyRange {
        start: 0x1D7E4,
        end: 0x1D7E5,
        value: 2,
    },
    PropertyRange {
        start: 0x1D7E5,
        end: 0x1D7E6,
        value: 3,
    },
    PropertyRange {
        start: 0x1D7E6,
        end: 0x1D7E7,
        value: 4,
    },
    PropertyRange {
        start: 0x1D7E7,
        end: 0x1D7E8,
        value: 5,
    },
    PropertyRange {
        start: 0x1D7E8,
        end: 0x1D7E9,
        value: 6,
    },
    PropertyRange {
        start: 0x1D7E9,
        end: 0x1D7EA,
        value: 7,
    },
    PropertyRange {
        start: 0x1D7EA,
        end: 0x1D7EB,
        value: 8,
    },
    PropertyRange {
        start: 0x1D7EB,
        end: 0x1D7EC,
        value: 9,
    },
    PropertyRange {
        start: 0x1D7EC,
        end: 0x1D7ED,
        value: 0,
    },
    PropertyRange {
        start: 0x1D7ED,
        end: 0x1D7EE,
        value: 1,
    },
    PropertyRange {
        start: 0x1D7EE,
        end: 0x1D7EF,
        value: 2,
    },
    PropertyRange {
        start: 0x1D7EF,
        end: 0x1D7F0,
        value: 3,
    },
    PropertyRange {
        start: 0x1D7F0,
        end: 0x1D7F1,
        value: 4,
    },
    PropertyRange {
        start: 0x1D7F1,
        end: 0x1D7F2,
        value: 5,
    },
    PropertyRange {
        start: 0x1D7F2,
        end: 0x1D7F3,
        value: 6,
    },
    PropertyRange {
        start: 0x1D7F3,
        end: 0x1D7F4,
        value: 7,
    },
    PropertyRange {
        start: 0x1D7F4,
        end: 0x1D7F5,
        value: 8,
    },
    PropertyRange {
        start: 0x1D7F5,
        end: 0x1D7F6,
        value: 9,
    },
    PropertyRange {
        start: 0x1D7F6,
        end: 0x1D7F7,
        value: 0,
    },
    PropertyRange {
        start: 0x1D7F7,
        end: 0x1D7F8,
        value: 1,
    },
    PropertyRange {
        start: 0x1D7F8,
        end: 0x1D7F9,
        value: 2,
    },
    PropertyRange {
        start: 0x1D7F9,
        end: 0x1D7FA,
        value: 3,
    },
    PropertyRange {
        start: 0x1D7FA,
        end: 0x1D7FB,
        value: 4,
    },
    PropertyRange {
        start: 0x1D7FB,
        end: 0x1D7FC,
        value: 5,
    },
    PropertyRange {
        start: 0x1D7FC,
        end: 0x1D7FD,
        value: 6,
    },
    PropertyRange {
        start: 0x1D7FD,
        end: 0x1D7FE,
        value: 7,
    },
    PropertyRange {
        start: 0x1D7FE,
        end: 0x1D7FF,
        value: 8,
    },
    PropertyRange {
        start: 0x1D7FF,
        end: 0x1D800,
        value: 9,
    },
    PropertyRange {
        start: 0x1E140,
        end: 0x1E141,
        value: 0,
    },
    PropertyRange {
        start: 0x1E141,
        end: 0x1E142,
        value: 1,
    },
    PropertyRange {
        start: 0x1E142,
        end: 0x1E143,
        value: 2,
    },
    PropertyRange {
        start: 0x1E143,
        end: 0x1E144,
        value: 3,
    },
    PropertyRange {
        start: 0x1E144,
        end: 0x1E145,
        value: 4,
    },
    PropertyRange {
        start: 0x1E145,
        end: 0x1E146,
        value: 5,
    },
    PropertyRange {
        start: 0x1E146,
        end: 0x1E147,
        value: 6,
    },
    PropertyRange {
        start: 0x1E147,
        end: 0x1E148,
        value: 7,
    },
    PropertyRange {
        start: 0x1E148,
        end: 0x1E149,
        value: 8,
    },
    PropertyRange {
        start: 0x1E149,
        end: 0x1E14A,
        value: 9,
    },
    PropertyRange {
        start: 0x1E2F0,
        end: 0x1E2F1,
        value: 0,
    },
    PropertyRange {
        start: 0x1E2F1,
        end: 0x1E2F2,
        value: 1,
    },
    PropertyRange {
        start: 0x1E2F2,
        end: 0x1E2F3,
        value: 2,
    },
    PropertyRange {
        start: 0x1E2F3,
        end: 0x1E2F4,
        value: 3,
    },
    PropertyRange {
        start: 0x1E2F4,
        end: 0x1E2F5,
        value: 4,
    },
    PropertyRange {
        start: 0x1E2F5,
        end: 0x1E2F6,
        value: 5,
    },
    PropertyRange {
        start: 0x1E2F6,
        end: 0x1E2F7,
        value: 6,
    },
    PropertyRange {
        start: 0x1E2F7,
        end: 0x1E2F8,
        value: 7,
    },
    PropertyRange {
        start: 0x1E2F8,
        end: 0x1E2F9,
        value: 8,
    },
    PropertyRange {
        start: 0x1E2F9,
        end: 0x1E2FA,
        value: 9,
    },
    PropertyRange {
        start: 0x1E4F0,
        end: 0x1E4F1,
        value: 0,
    },
    PropertyRange {
        start: 0x1E4F1,
        end: 0x1E4F2,
        value: 1,
    },
    PropertyRange {
        start: 0x1E4F2,
        end: 0x1E4F3,
        value: 2,
    },
    PropertyRange {
        start: 0x1E4F3,
        end: 0x1E4F4,
        value: 3,
    },
    PropertyRange {
        start: 0x1E4F4,
        end: 0x1E4F5,
        value: 4,
    },
    PropertyRange {
        start: 0x1E4F5,
        end: 0x1E4F6,
        value: 5,
    },
    PropertyRange {
        start: 0x1E4F6,
        end: 0x1E4F7,
        value: 6,
    },
    PropertyRange {
        start: 0x1E4F7,
        end: 0x1E4F8,
        value: 7,
    },
    PropertyRange {
        start: 0x1E4F8,
        end: 0x1E4F9,
        value: 8,
    },
    PropertyRange {
        start: 0x1E4F9,
        end: 0x1E4FA,
        value: 9,
    },
    PropertyRange {
        start: 0x1E5F1,
        end: 0x1E5F2,
        value: 0,
    },
    PropertyRange {
        start: 0x1E5F2,
        end: 0x1E5F3,
        value: 1,
    },
    PropertyRange {
        start: 0x1E5F3,
        end: 0x1E5F4,
        value: 2,
    },
    PropertyRange {
        start: 0x1E5F4,
        end: 0x1E5F5,
        value: 3,
    },
    PropertyRange {
        start: 0x1E5F5,
        end: 0x1E5F6,
        value: 4,
    },
    PropertyRange {
        start: 0x1E5F6,
        end: 0x1E5F7,
        value: 5,
    },
    PropertyRange {
        start: 0x1E5F7,
        end: 0x1E5F8,
        value: 6,
    },
    PropertyRange {
        start: 0x1E5F8,
        end: 0x1E5F9,
        value: 7,
    },
    PropertyRange {
        start: 0x1E5F9,
        end: 0x1E5FA,
        value: 8,
    },
    PropertyRange {
        start: 0x1E5FA,
        end: 0x1E5FB,
        value: 9,
    },
    PropertyRange {
        start: 0x1E950,
        end: 0x1E951,
        value: 0,
    },
    PropertyRange {
        start: 0x1E951,
        end: 0x1E952,
        value: 1,
    },
    PropertyRange {
        start: 0x1E952,
        end: 0x1E953,
        value: 2,
    },
    PropertyRange {
        start: 0x1E953,
        end: 0x1E954,
        value: 3,
    },
    PropertyRange {
        start: 0x1E954,
        end: 0x1E955,
        value: 4,
    },
    PropertyRange {
        start: 0x1E955,
        end: 0x1E956,
        value: 5,
    },
    PropertyRange {
        start: 0x1E956,
        end: 0x1E957,
        value: 6,
    },
    PropertyRange {
        start: 0x1E957,
        end: 0x1E958,
        value: 7,
    },
    PropertyRange {
        start: 0x1E958,
        end: 0x1E959,
        value: 8,
    },
    PropertyRange {
        start: 0x1E959,
        end: 0x1E95A,
        value: 9,
    },
    PropertyRange {
        start: 0x1FBF0,
        end: 0x1FBF1,
        value: 0,
    },
    PropertyRange {
        start: 0x1FBF1,
        end: 0x1FBF2,
        value: 1,
    },
    PropertyRange {
        start: 0x1FBF2,
        end: 0x1FBF3,
        value: 2,
    },
    PropertyRange {
        start: 0x1FBF3,
        end: 0x1FBF4,
        value: 3,
    },
    PropertyRange {
        start: 0x1FBF4,
        end: 0x1FBF5,
        value: 4,
    },
    PropertyRange {
        start: 0x1FBF5,
        end: 0x1FBF6,
        value: 5,
    },
    PropertyRange {
        start: 0x1FBF6,
        end: 0x1FBF7,
        value: 6,
    },
    PropertyRange {
        start: 0x1FBF7,
        end: 0x1FBF8,
        value: 7,
    },
    PropertyRange {
        start: 0x1FBF8,
        end: 0x1FBF9,
        value: 8,
    },
    PropertyRange {
        start: 0x1FBF9,
        end: 0x1FBFA,
        value: 9,
    },
];

pub const MARKS: &[Range] = &[
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
        end: 0x904,
    },
    Range {
        start: 0x93A,
        end: 0x93D,
    },
    Range {
        start: 0x93E,
        end: 0x950,
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
        end: 0x984,
    },
    Range {
        start: 0x9BC,
        end: 0x9BD,
    },
    Range {
        start: 0x9BE,
        end: 0x9C5,
    },
    Range {
        start: 0x9C7,
        end: 0x9C9,
    },
    Range {
        start: 0x9CB,
        end: 0x9CE,
    },
    Range {
        start: 0x9D7,
        end: 0x9D8,
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
        end: 0xA04,
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
        start: 0xA70,
        end: 0xA72,
    },
    Range {
        start: 0xA75,
        end: 0xA76,
    },
    Range {
        start: 0xA81,
        end: 0xA84,
    },
    Range {
        start: 0xABC,
        end: 0xABD,
    },
    Range {
        start: 0xABE,
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
        start: 0xAE2,
        end: 0xAE4,
    },
    Range {
        start: 0xAFA,
        end: 0xB00,
    },
    Range {
        start: 0xB01,
        end: 0xB04,
    },
    Range {
        start: 0xB3C,
        end: 0xB3D,
    },
    Range {
        start: 0xB3E,
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
        start: 0xB62,
        end: 0xB64,
    },
    Range {
        start: 0xB82,
        end: 0xB83,
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
        start: 0xBD7,
        end: 0xBD8,
    },
    Range {
        start: 0xC00,
        end: 0xC05,
    },
    Range {
        start: 0xC3C,
        end: 0xC3D,
    },
    Range {
        start: 0xC3E,
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
        start: 0xC62,
        end: 0xC64,
    },
    Range {
        start: 0xC81,
        end: 0xC84,
    },
    Range {
        start: 0xCBC,
        end: 0xCBD,
    },
    Range {
        start: 0xCBE,
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
        start: 0xCE2,
        end: 0xCE4,
    },
    Range {
        start: 0xCF3,
        end: 0xCF4,
    },
    Range {
        start: 0xD00,
        end: 0xD04,
    },
    Range {
        start: 0xD3B,
        end: 0xD3D,
    },
    Range {
        start: 0xD3E,
        end: 0xD45,
    },
    Range {
        start: 0xD46,
        end: 0xD49,
    },
    Range {
        start: 0xD4A,
        end: 0xD4E,
    },
    Range {
        start: 0xD57,
        end: 0xD58,
    },
    Range {
        start: 0xD62,
        end: 0xD64,
    },
    Range {
        start: 0xD81,
        end: 0xD84,
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
        start: 0xDF2,
        end: 0xDF4,
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
        start: 0xF3E,
        end: 0xF40,
    },
    Range {
        start: 0xF71,
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
        start: 0x102B,
        end: 0x103F,
    },
    Range {
        start: 0x1056,
        end: 0x105A,
    },
    Range {
        start: 0x105E,
        end: 0x1061,
    },
    Range {
        start: 0x1062,
        end: 0x1065,
    },
    Range {
        start: 0x1067,
        end: 0x106E,
    },
    Range {
        start: 0x1071,
        end: 0x1075,
    },
    Range {
        start: 0x1082,
        end: 0x108E,
    },
    Range {
        start: 0x108F,
        end: 0x1090,
    },
    Range {
        start: 0x109A,
        end: 0x109E,
    },
    Range {
        start: 0x135D,
        end: 0x1360,
    },
    Range {
        start: 0x1712,
        end: 0x1716,
    },
    Range {
        start: 0x1732,
        end: 0x1735,
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
        end: 0x192C,
    },
    Range {
        start: 0x1930,
        end: 0x193C,
    },
    Range {
        start: 0x1A17,
        end: 0x1A1C,
    },
    Range {
        start: 0x1A55,
        end: 0x1A5F,
    },
    Range {
        start: 0x1A60,
        end: 0x1A7D,
    },
    Range {
        start: 0x1A7F,
        end: 0x1A80,
    },
    Range {
        start: 0x1AB0,
        end: 0x1ACF,
    },
    Range {
        start: 0x1B00,
        end: 0x1B05,
    },
    Range {
        start: 0x1B34,
        end: 0x1B45,
    },
    Range {
        start: 0x1B6B,
        end: 0x1B74,
    },
    Range {
        start: 0x1B80,
        end: 0x1B83,
    },
    Range {
        start: 0x1BA1,
        end: 0x1BAE,
    },
    Range {
        start: 0x1BE6,
        end: 0x1BF4,
    },
    Range {
        start: 0x1C24,
        end: 0x1C38,
    },
    Range {
        start: 0x1CD0,
        end: 0x1CD3,
    },
    Range {
        start: 0x1CD4,
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
        start: 0x1CF7,
        end: 0x1CFA,
    },
    Range {
        start: 0x1DC0,
        end: 0x1E00,
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
        end: 0x3030,
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
        start: 0xA823,
        end: 0xA828,
    },
    Range {
        start: 0xA82C,
        end: 0xA82D,
    },
    Range {
        start: 0xA880,
        end: 0xA882,
    },
    Range {
        start: 0xA8B4,
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
        end: 0xA954,
    },
    Range {
        start: 0xA980,
        end: 0xA984,
    },
    Range {
        start: 0xA9B3,
        end: 0xA9C1,
    },
    Range {
        start: 0xA9E5,
        end: 0xA9E6,
    },
    Range {
        start: 0xAA29,
        end: 0xAA37,
    },
    Range {
        start: 0xAA43,
        end: 0xAA44,
    },
    Range {
        start: 0xAA4C,
        end: 0xAA4E,
    },
    Range {
        start: 0xAA7B,
        end: 0xAA7E,
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
        start: 0xAAEB,
        end: 0xAAF0,
    },
    Range {
        start: 0xAAF5,
        end: 0xAAF7,
    },
    Range {
        start: 0xABE3,
        end: 0xABEB,
    },
    Range {
        start: 0xABEC,
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
        start: 0x10EFC,
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
        start: 0x11000,
        end: 0x11003,
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
        end: 0x11083,
    },
    Range {
        start: 0x110B0,
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
        end: 0x11135,
    },
    Range {
        start: 0x11145,
        end: 0x11147,
    },
    Range {
        start: 0x11173,
        end: 0x11174,
    },
    Range {
        start: 0x11180,
        end: 0x11183,
    },
    Range {
        start: 0x111B3,
        end: 0x111C1,
    },
    Range {
        start: 0x111C9,
        end: 0x111CD,
    },
    Range {
        start: 0x111CE,
        end: 0x111D0,
    },
    Range {
        start: 0x1122C,
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
        end: 0x112EB,
    },
    Range {
        start: 0x11300,
        end: 0x11304,
    },
    Range {
        start: 0x1133B,
        end: 0x1133D,
    },
    Range {
        start: 0x1133E,
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
        start: 0x11357,
        end: 0x11358,
    },
    Range {
        start: 0x11362,
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
        start: 0x113B8,
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
        start: 0x11435,
        end: 0x11447,
    },
    Range {
        start: 0x1145E,
        end: 0x1145F,
    },
    Range {
        start: 0x114B0,
        end: 0x114C4,
    },
    Range {
        start: 0x115AF,
        end: 0x115B6,
    },
    Range {
        start: 0x115B8,
        end: 0x115C1,
    },
    Range {
        start: 0x115DC,
        end: 0x115DE,
    },
    Range {
        start: 0x11630,
        end: 0x11641,
    },
    Range {
        start: 0x116AB,
        end: 0x116B8,
    },
    Range {
        start: 0x1171D,
        end: 0x1172C,
    },
    Range {
        start: 0x1182C,
        end: 0x1183B,
    },
    Range {
        start: 0x11930,
        end: 0x11936,
    },
    Range {
        start: 0x11937,
        end: 0x11939,
    },
    Range {
        start: 0x1193B,
        end: 0x1193F,
    },
    Range {
        start: 0x11940,
        end: 0x11941,
    },
    Range {
        start: 0x11942,
        end: 0x11944,
    },
    Range {
        start: 0x119D1,
        end: 0x119D8,
    },
    Range {
        start: 0x119DA,
        end: 0x119E1,
    },
    Range {
        start: 0x119E4,
        end: 0x119E5,
    },
    Range {
        start: 0x11A01,
        end: 0x11A0B,
    },
    Range {
        start: 0x11A33,
        end: 0x11A3A,
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
        end: 0x11A5C,
    },
    Range {
        start: 0x11A8A,
        end: 0x11A9A,
    },
    Range {
        start: 0x11C2F,
        end: 0x11C37,
    },
    Range {
        start: 0x11C38,
        end: 0x11C40,
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
        start: 0x11D8A,
        end: 0x11D8F,
    },
    Range {
        start: 0x11D90,
        end: 0x11D92,
    },
    Range {
        start: 0x11D93,
        end: 0x11D98,
    },
    Range {
        start: 0x11EF3,
        end: 0x11EF7,
    },
    Range {
        start: 0x11F00,
        end: 0x11F02,
    },
    Range {
        start: 0x11F03,
        end: 0x11F04,
    },
    Range {
        start: 0x11F34,
        end: 0x11F3B,
    },
    Range {
        start: 0x11F3E,
        end: 0x11F43,
    },
    Range {
        start: 0x11F5A,
        end: 0x11F5B,
    },
    Range {
        start: 0x13440,
        end: 0x13441,
    },
    Range {
        start: 0x13447,
        end: 0x13456,
    },
    Range {
        start: 0x1611E,
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
        start: 0x16F51,
        end: 0x16F88,
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
        start: 0x16FF0,
        end: 0x16FF2,
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
        start: 0x1D165,
        end: 0x1D16A,
    },
    Range {
        start: 0x1D16D,
        end: 0x1D173,
    },
    Range {
        start: 0x1D17B,
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
        start: 0x1E8D0,
        end: 0x1E8D7,
    },
    Range {
        start: 0x1E944,
        end: 0x1E94B,
    },
    Range {
        start: 0xE0100,
        end: 0xE01F0,
    },
];

pub const NAMED: &[Range] = &[
    Range {
        start: 0x20,
        end: 0x7F,
    },
    Range {
        start: 0xA0,
        end: 0x378,
    },
    Range {
        start: 0x37A,
        end: 0x380,
    },
    Range {
        start: 0x384,
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
        end: 0x530,
    },
    Range {
        start: 0x531,
        end: 0x557,
    },
    Range {
        start: 0x559,
        end: 0x58B,
    },
    Range {
        start: 0x58D,
        end: 0x590,
    },
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
        start: 0x600,
        end: 0x70E,
    },
    Range {
        start: 0x70F,
        end: 0x74B,
    },
    Range {
        start: 0x74D,
        end: 0x7B2,
    },
    Range {
        start: 0x7C0,
        end: 0x7FB,
    },
    Range {
        start: 0x7FD,
        end: 0x82E,
    },
    Range {
        start: 0x830,
        end: 0x83F,
    },
    Range {
        start: 0x840,
        end: 0x85C,
    },
    Range {
        start: 0x85E,
        end: 0x85F,
    },
    Range {
        start: 0x860,
        end: 0x86B,
    },
    Range {
        start: 0x870,
        end: 0x88F,
    },
    Range {
        start: 0x890,
        end: 0x892,
    },
    Range {
        start: 0x897,
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
        start: 0x9DC,
        end: 0x9DE,
    },
    Range {
        start: 0x9DF,
        end: 0x9E4,
    },
    Range {
        start: 0x9E6,
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
        end: 0xA34,
    },
    Range {
        start: 0xA35,
        end: 0xA37,
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
        start: 0xA59,
        end: 0xA5D,
    },
    Range {
        start: 0xA5E,
        end: 0xA5F,
    },
    Range {
        start: 0xA66,
        end: 0xA77,
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
        end: 0xAF2,
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
        start: 0xB5C,
        end: 0xB5E,
    },
    Range {
        start: 0xB5F,
        end: 0xB64,
    },
    Range {
        start: 0xB66,
        end: 0xB78,
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
        end: 0xBFB,
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
        start: 0xC5D,
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
        start: 0xC77,
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
        start: 0xCDD,
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
        end: 0xD50,
    },
    Range {
        start: 0xD54,
        end: 0xD64,
    },
    Range {
        start: 0xD66,
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
        end: 0xDF5,
    },
    Range {
        start: 0xE01,
        end: 0xE3B,
    },
    Range {
        start: 0xE3F,
        end: 0xE5C,
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
        start: 0xEDC,
        end: 0xEE0,
    },
    Range {
        start: 0xF00,
        end: 0xF48,
    },
    Range {
        start: 0xF49,
        end: 0xF6D,
    },
    Range {
        start: 0xF71,
        end: 0xF98,
    },
    Range {
        start: 0xF99,
        end: 0xFBD,
    },
    Range {
        start: 0xFBE,
        end: 0xFCD,
    },
    Range {
        start: 0xFCE,
        end: 0xFDB,
    },
    Range {
        start: 0x1000,
        end: 0x10C6,
    },
    Range {
        start: 0x10C7,
        end: 0x10C8,
    },
    Range {
        start: 0x10CD,
        end: 0x10CE,
    },
    Range {
        start: 0x10D0,
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
        end: 0x137D,
    },
    Range {
        start: 0x1380,
        end: 0x139A,
    },
    Range {
        start: 0x13A0,
        end: 0x13F6,
    },
    Range {
        start: 0x13F8,
        end: 0x13FE,
    },
    Range {
        start: 0x1400,
        end: 0x169D,
    },
    Range {
        start: 0x16A0,
        end: 0x16F9,
    },
    Range {
        start: 0x1700,
        end: 0x1716,
    },
    Range {
        start: 0x171F,
        end: 0x1737,
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
        end: 0x17DE,
    },
    Range {
        start: 0x17E0,
        end: 0x17EA,
    },
    Range {
        start: 0x17F0,
        end: 0x17FA,
    },
    Range {
        start: 0x1800,
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
        start: 0x1940,
        end: 0x1941,
    },
    Range {
        start: 0x1944,
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
        end: 0x19DB,
    },
    Range {
        start: 0x19DE,
        end: 0x1A1C,
    },
    Range {
        start: 0x1A1E,
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
        start: 0x1AA0,
        end: 0x1AAE,
    },
    Range {
        start: 0x1AB0,
        end: 0x1ACF,
    },
    Range {
        start: 0x1B00,
        end: 0x1B4D,
    },
    Range {
        start: 0x1B4E,
        end: 0x1BF4,
    },
    Range {
        start: 0x1BFC,
        end: 0x1C38,
    },
    Range {
        start: 0x1C3B,
        end: 0x1C4A,
    },
    Range {
        start: 0x1C4D,
        end: 0x1C8B,
    },
    Range {
        start: 0x1C90,
        end: 0x1CBB,
    },
    Range {
        start: 0x1CBD,
        end: 0x1CC8,
    },
    Range {
        start: 0x1CD0,
        end: 0x1CFB,
    },
    Range {
        start: 0x1D00,
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
        start: 0x2000,
        end: 0x2065,
    },
    Range {
        start: 0x2066,
        end: 0x2072,
    },
    Range {
        start: 0x2074,
        end: 0x208F,
    },
    Range {
        start: 0x2090,
        end: 0x209D,
    },
    Range {
        start: 0x20A0,
        end: 0x20C1,
    },
    Range {
        start: 0x20D0,
        end: 0x20F1,
    },
    Range {
        start: 0x2100,
        end: 0x218C,
    },
    Range {
        start: 0x2190,
        end: 0x242A,
    },
    Range {
        start: 0x2440,
        end: 0x244B,
    },
    Range {
        start: 0x2460,
        end: 0x2B74,
    },
    Range {
        start: 0x2B76,
        end: 0x2B96,
    },
    Range {
        start: 0x2B97,
        end: 0x2CF4,
    },
    Range {
        start: 0x2CF9,
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
        start: 0x2D6F,
        end: 0x2D71,
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
        end: 0x2E5E,
    },
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
        start: 0x2FF0,
        end: 0x3040,
    },
    Range {
        start: 0x3041,
        end: 0x3097,
    },
    Range {
        start: 0x3099,
        end: 0x3100,
    },
    Range {
        start: 0x3105,
        end: 0x3130,
    },
    Range {
        start: 0x3131,
        end: 0x318F,
    },
    Range {
        start: 0x3190,
        end: 0x31E6,
    },
    Range {
        start: 0x31EF,
        end: 0x321F,
    },
    Range {
        start: 0x3220,
        end: 0xA48D,
    },
    Range {
        start: 0xA490,
        end: 0xA4C7,
    },
    Range {
        start: 0xA4D0,
        end: 0xA62C,
    },
    Range {
        start: 0xA640,
        end: 0xA6F8,
    },
    Range {
        start: 0xA700,
        end: 0xA7CE,
    },
    Range {
        start: 0xA7D0,
        end: 0xA7D2,
    },
    Range {
        start: 0xA7D3,
        end: 0xA7D4,
    },
    Range {
        start: 0xA7D5,
        end: 0xA7DD,
    },
    Range {
        start: 0xA7F2,
        end: 0xA82D,
    },
    Range {
        start: 0xA830,
        end: 0xA83A,
    },
    Range {
        start: 0xA840,
        end: 0xA878,
    },
    Range {
        start: 0xA880,
        end: 0xA8C6,
    },
    Range {
        start: 0xA8CE,
        end: 0xA8DA,
    },
    Range {
        start: 0xA8E0,
        end: 0xA954,
    },
    Range {
        start: 0xA95F,
        end: 0xA97D,
    },
    Range {
        start: 0xA980,
        end: 0xA9CE,
    },
    Range {
        start: 0xA9CF,
        end: 0xA9DA,
    },
    Range {
        start: 0xA9DE,
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
        start: 0xAA5C,
        end: 0xAAC3,
    },
    Range {
        start: 0xAADB,
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
        end: 0xAB6C,
    },
    Range {
        start: 0xAB70,
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
        start: 0xD7B0,
        end: 0xD7C7,
    },
    Range {
        start: 0xD7CB,
        end: 0xD7FC,
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
        start: 0xFB00,
        end: 0xFB07,
    },
    Range {
        start: 0xFB13,
        end: 0xFB18,
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
        end: 0xFBC3,
    },
    Range {
        start: 0xFBD3,
        end: 0xFD90,
    },
    Range {
        start: 0xFD92,
        end: 0xFDC8,
    },
    Range {
        start: 0xFDCF,
        end: 0xFDD0,
    },
    Range {
        start: 0xFDF0,
        end: 0xFE1A,
    },
    Range {
        start: 0xFE20,
        end: 0xFE53,
    },
    Range {
        start: 0xFE54,
        end: 0xFE67,
    },
    Range {
        start: 0xFE68,
        end: 0xFE6C,
    },
    Range {
        start: 0xFE70,
        end: 0xFE75,
    },
    Range {
        start: 0xFE76,
        end: 0xFEFD,
    },
    Range {
        start: 0xFEFF,
        end: 0xFF00,
    },
    Range {
        start: 0xFF01,
        end: 0xFFBF,
    },
    Range {
        start: 0xFFC2,
        end: 0xFFC8,
    },
    Range {
        start: 0xFFCA,
        end: 0xFFD0,
    },
    Range {
        start: 0xFFD2,
        end: 0xFFD8,
    },
    Range {
        start: 0xFFDA,
        end: 0xFFDD,
    },
    Range {
        start: 0xFFE0,
        end: 0xFFE7,
    },
    Range {
        start: 0xFFE8,
        end: 0xFFEF,
    },
    Range {
        start: 0xFFF9,
        end: 0xFFFE,
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
        start: 0x10100,
        end: 0x10103,
    },
    Range {
        start: 0x10107,
        end: 0x10134,
    },
    Range {
        start: 0x10137,
        end: 0x1018F,
    },
    Range {
        start: 0x10190,
        end: 0x1019D,
    },
    Range {
        start: 0x101A0,
        end: 0x101A1,
    },
    Range {
        start: 0x101D0,
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
        end: 0x102FC,
    },
    Range {
        start: 0x10300,
        end: 0x10324,
    },
    Range {
        start: 0x1032D,
        end: 0x1034B,
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
        start: 0x1039F,
        end: 0x103C4,
    },
    Range {
        start: 0x103C8,
        end: 0x103D6,
    },
    Range {
        start: 0x10400,
        end: 0x1049E,
    },
    Range {
        start: 0x104A0,
        end: 0x104AA,
    },
    Range {
        start: 0x104B0,
        end: 0x104D4,
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
        start: 0x1056F,
        end: 0x1057B,
    },
    Range {
        start: 0x1057C,
        end: 0x1058B,
    },
    Range {
        start: 0x1058C,
        end: 0x10593,
    },
    Range {
        start: 0x10594,
        end: 0x10596,
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
        end: 0x10786,
    },
    Range {
        start: 0x10787,
        end: 0x107B1,
    },
    Range {
        start: 0x107B2,
        end: 0x107BB,
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
        start: 0x10857,
        end: 0x1089F,
    },
    Range {
        start: 0x108A7,
        end: 0x108B0,
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
        start: 0x108FB,
        end: 0x1091C,
    },
    Range {
        start: 0x1091F,
        end: 0x1093A,
    },
    Range {
        start: 0x1093F,
        end: 0x10940,
    },
    Range {
        start: 0x10980,
        end: 0x109B8,
    },
    Range {
        start: 0x109BC,
        end: 0x109D0,
    },
    Range {
        start: 0x109D2,
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
        end: 0x10A49,
    },
    Range {
        start: 0x10A50,
        end: 0x10A59,
    },
    Range {
        start: 0x10A60,
        end: 0x10AA0,
    },
    Range {
        start: 0x10AC0,
        end: 0x10AE7,
    },
    Range {
        start: 0x10AEB,
        end: 0x10AF7,
    },
    Range {
        start: 0x10B00,
        end: 0x10B36,
    },
    Range {
        start: 0x10B39,
        end: 0x10B56,
    },
    Range {
        start: 0x10B58,
        end: 0x10B73,
    },
    Range {
        start: 0x10B78,
        end: 0x10B92,
    },
    Range {
        start: 0x10B99,
        end: 0x10B9D,
    },
    Range {
        start: 0x10BA9,
        end: 0x10BB0,
    },
    Range {
        start: 0x10C00,
        end: 0x10C49,
    },
    Range {
        start: 0x10C80,
        end: 0x10CB3,
    },
    Range {
        start: 0x10CC0,
        end: 0x10CF3,
    },
    Range {
        start: 0x10CFA,
        end: 0x10D28,
    },
    Range {
        start: 0x10D30,
        end: 0x10D3A,
    },
    Range {
        start: 0x10D40,
        end: 0x10D66,
    },
    Range {
        start: 0x10D69,
        end: 0x10D86,
    },
    Range {
        start: 0x10D8E,
        end: 0x10D90,
    },
    Range {
        start: 0x10E60,
        end: 0x10E7F,
    },
    Range {
        start: 0x10E80,
        end: 0x10EAA,
    },
    Range {
        start: 0x10EAB,
        end: 0x10EAE,
    },
    Range {
        start: 0x10EB0,
        end: 0x10EB2,
    },
    Range {
        start: 0x10EC2,
        end: 0x10EC5,
    },
    Range {
        start: 0x10EFC,
        end: 0x10F28,
    },
    Range {
        start: 0x10F30,
        end: 0x10F5A,
    },
    Range {
        start: 0x10F70,
        end: 0x10F8A,
    },
    Range {
        start: 0x10FB0,
        end: 0x10FCC,
    },
    Range {
        start: 0x10FE0,
        end: 0x10FF7,
    },
    Range {
        start: 0x11000,
        end: 0x1104E,
    },
    Range {
        start: 0x11052,
        end: 0x11076,
    },
    Range {
        start: 0x1107F,
        end: 0x110C3,
    },
    Range {
        start: 0x110CD,
        end: 0x110CE,
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
        end: 0x11148,
    },
    Range {
        start: 0x11150,
        end: 0x11177,
    },
    Range {
        start: 0x11180,
        end: 0x111E0,
    },
    Range {
        start: 0x111E1,
        end: 0x111F5,
    },
    Range {
        start: 0x11200,
        end: 0x11212,
    },
    Range {
        start: 0x11213,
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
        end: 0x112AA,
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
        end: 0x113D6,
    },
    Range {
        start: 0x113D7,
        end: 0x113D9,
    },
    Range {
        start: 0x113E1,
        end: 0x113E3,
    },
    Range {
        start: 0x11400,
        end: 0x1145C,
    },
    Range {
        start: 0x1145D,
        end: 0x11462,
    },
    Range {
        start: 0x11480,
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
        end: 0x115DE,
    },
    Range {
        start: 0x11600,
        end: 0x11645,
    },
    Range {
        start: 0x11650,
        end: 0x1165A,
    },
    Range {
        start: 0x11660,
        end: 0x1166D,
    },
    Range {
        start: 0x11680,
        end: 0x116BA,
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
        end: 0x11747,
    },
    Range {
        start: 0x11800,
        end: 0x1183C,
    },
    Range {
        start: 0x118A0,
        end: 0x118F3,
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
        end: 0x11947,
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
        end: 0x119E5,
    },
    Range {
        start: 0x11A00,
        end: 0x11A48,
    },
    Range {
        start: 0x11A50,
        end: 0x11AA3,
    },
    Range {
        start: 0x11AB0,
        end: 0x11AF9,
    },
    Range {
        start: 0x11B00,
        end: 0x11B0A,
    },
    Range {
        start: 0x11BC0,
        end: 0x11BE2,
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
        end: 0x11C46,
    },
    Range {
        start: 0x11C50,
        end: 0x11C6D,
    },
    Range {
        start: 0x11C70,
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
        start: 0x11EE0,
        end: 0x11EF9,
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
        end: 0x11F5B,
    },
    Range {
        start: 0x11FB0,
        end: 0x11FB1,
    },
    Range {
        start: 0x11FC0,
        end: 0x11FF2,
    },
    Range {
        start: 0x11FFF,
        end: 0x1239A,
    },
    Range {
        start: 0x12400,
        end: 0x1246F,
    },
    Range {
        start: 0x12470,
        end: 0x12475,
    },
    Range {
        start: 0x12480,
        end: 0x12544,
    },
    Range {
        start: 0x12F90,
        end: 0x12FF3,
    },
    Range {
        start: 0x13000,
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
        start: 0x16A6E,
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
        end: 0x16AF6,
    },
    Range {
        start: 0x16B00,
        end: 0x16B46,
    },
    Range {
        start: 0x16B50,
        end: 0x16B5A,
    },
    Range {
        start: 0x16B5B,
        end: 0x16B62,
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
        end: 0x16D7A,
    },
    Range {
        start: 0x16E40,
        end: 0x16E9B,
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
        end: 0x16FE5,
    },
    Range {
        start: 0x16FF0,
        end: 0x16FF2,
    },
    Range {
        start: 0x18800,
        end: 0x18CD6,
    },
    Range {
        start: 0x18CFF,
        end: 0x18D00,
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
        start: 0x1BC9C,
        end: 0x1BCA4,
    },
    Range {
        start: 0x1CC00,
        end: 0x1CCFA,
    },
    Range {
        start: 0x1CD00,
        end: 0x1CEB4,
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
        start: 0x1CF50,
        end: 0x1CFC4,
    },
    Range {
        start: 0x1D000,
        end: 0x1D0F6,
    },
    Range {
        start: 0x1D100,
        end: 0x1D127,
    },
    Range {
        start: 0x1D129,
        end: 0x1D1EB,
    },
    Range {
        start: 0x1D200,
        end: 0x1D246,
    },
    Range {
        start: 0x1D2C0,
        end: 0x1D2D4,
    },
    Range {
        start: 0x1D2E0,
        end: 0x1D2F4,
    },
    Range {
        start: 0x1D300,
        end: 0x1D357,
    },
    Range {
        start: 0x1D360,
        end: 0x1D379,
    },
    Range {
        start: 0x1D400,
        end: 0x1D455,
    },
    Range {
        start: 0x1D456,
        end: 0x1D49D,
    },
    Range {
        start: 0x1D49E,
        end: 0x1D4A0,
    },
    Range {
        start: 0x1D4A2,
        end: 0x1D4A3,
    },
    Range {
        start: 0x1D4A5,
        end: 0x1D4A7,
    },
    Range {
        start: 0x1D4A9,
        end: 0x1D4AD,
    },
    Range {
        start: 0x1D4AE,
        end: 0x1D4BA,
    },
    Range {
        start: 0x1D4BB,
        end: 0x1D4BC,
    },
    Range {
        start: 0x1D4BD,
        end: 0x1D4C4,
    },
    Range {
        start: 0x1D4C5,
        end: 0x1D506,
    },
    Range {
        start: 0x1D507,
        end: 0x1D50B,
    },
    Range {
        start: 0x1D50D,
        end: 0x1D515,
    },
    Range {
        start: 0x1D516,
        end: 0x1D51D,
    },
    Range {
        start: 0x1D51E,
        end: 0x1D53A,
    },
    Range {
        start: 0x1D53B,
        end: 0x1D53F,
    },
    Range {
        start: 0x1D540,
        end: 0x1D545,
    },
    Range {
        start: 0x1D546,
        end: 0x1D547,
    },
    Range {
        start: 0x1D54A,
        end: 0x1D551,
    },
    Range {
        start: 0x1D552,
        end: 0x1D6A6,
    },
    Range {
        start: 0x1D6A8,
        end: 0x1D7CC,
    },
    Range {
        start: 0x1D7CE,
        end: 0x1DA8C,
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
        start: 0x1E030,
        end: 0x1E06E,
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
        end: 0x1E150,
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
        start: 0x1E2FF,
        end: 0x1E300,
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
        start: 0x1E5FF,
        end: 0x1E600,
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
        start: 0x1E8C7,
        end: 0x1E8D7,
    },
    Range {
        start: 0x1E900,
        end: 0x1E94C,
    },
    Range {
        start: 0x1E950,
        end: 0x1E95A,
    },
    Range {
        start: 0x1E95E,
        end: 0x1E960,
    },
    Range {
        start: 0x1EC71,
        end: 0x1ECB5,
    },
    Range {
        start: 0x1ED01,
        end: 0x1ED3E,
    },
    Range {
        start: 0x1EE00,
        end: 0x1EE04,
    },
    Range {
        start: 0x1EE05,
        end: 0x1EE20,
    },
    Range {
        start: 0x1EE21,
        end: 0x1EE23,
    },
    Range {
        start: 0x1EE24,
        end: 0x1EE25,
    },
    Range {
        start: 0x1EE27,
        end: 0x1EE28,
    },
    Range {
        start: 0x1EE29,
        end: 0x1EE33,
    },
    Range {
        start: 0x1EE34,
        end: 0x1EE38,
    },
    Range {
        start: 0x1EE39,
        end: 0x1EE3A,
    },
    Range {
        start: 0x1EE3B,
        end: 0x1EE3C,
    },
    Range {
        start: 0x1EE42,
        end: 0x1EE43,
    },
    Range {
        start: 0x1EE47,
        end: 0x1EE48,
    },
    Range {
        start: 0x1EE49,
        end: 0x1EE4A,
    },
    Range {
        start: 0x1EE4B,
        end: 0x1EE4C,
    },
    Range {
        start: 0x1EE4D,
        end: 0x1EE50,
    },
    Range {
        start: 0x1EE51,
        end: 0x1EE53,
    },
    Range {
        start: 0x1EE54,
        end: 0x1EE55,
    },
    Range {
        start: 0x1EE57,
        end: 0x1EE58,
    },
    Range {
        start: 0x1EE59,
        end: 0x1EE5A,
    },
    Range {
        start: 0x1EE5B,
        end: 0x1EE5C,
    },
    Range {
        start: 0x1EE5D,
        end: 0x1EE5E,
    },
    Range {
        start: 0x1EE5F,
        end: 0x1EE60,
    },
    Range {
        start: 0x1EE61,
        end: 0x1EE63,
    },
    Range {
        start: 0x1EE64,
        end: 0x1EE65,
    },
    Range {
        start: 0x1EE67,
        end: 0x1EE6B,
    },
    Range {
        start: 0x1EE6C,
        end: 0x1EE73,
    },
    Range {
        start: 0x1EE74,
        end: 0x1EE78,
    },
    Range {
        start: 0x1EE79,
        end: 0x1EE7D,
    },
    Range {
        start: 0x1EE7E,
        end: 0x1EE7F,
    },
    Range {
        start: 0x1EE80,
        end: 0x1EE8A,
    },
    Range {
        start: 0x1EE8B,
        end: 0x1EE9C,
    },
    Range {
        start: 0x1EEA1,
        end: 0x1EEA4,
    },
    Range {
        start: 0x1EEA5,
        end: 0x1EEAA,
    },
    Range {
        start: 0x1EEAB,
        end: 0x1EEBC,
    },
    Range {
        start: 0x1EEF0,
        end: 0x1EEF2,
    },
    Range {
        start: 0x1F000,
        end: 0x1F02C,
    },
    Range {
        start: 0x1F030,
        end: 0x1F094,
    },
    Range {
        start: 0x1F0A0,
        end: 0x1F0AF,
    },
    Range {
        start: 0x1F0B1,
        end: 0x1F0C0,
    },
    Range {
        start: 0x1F0C1,
        end: 0x1F0D0,
    },
    Range {
        start: 0x1F0D1,
        end: 0x1F0F6,
    },
    Range {
        start: 0x1F100,
        end: 0x1F1AE,
    },
    Range {
        start: 0x1F1E6,
        end: 0x1F203,
    },
    Range {
        start: 0x1F210,
        end: 0x1F23C,
    },
    Range {
        start: 0x1F240,
        end: 0x1F249,
    },
    Range {
        start: 0x1F250,
        end: 0x1F252,
    },
    Range {
        start: 0x1F260,
        end: 0x1F266,
    },
    Range {
        start: 0x1F300,
        end: 0x1F6D8,
    },
    Range {
        start: 0x1F6DC,
        end: 0x1F6ED,
    },
    Range {
        start: 0x1F6F0,
        end: 0x1F6FD,
    },
    Range {
        start: 0x1F700,
        end: 0x1F777,
    },
    Range {
        start: 0x1F77B,
        end: 0x1F7DA,
    },
    Range {
        start: 0x1F7E0,
        end: 0x1F7EC,
    },
    Range {
        start: 0x1F7F0,
        end: 0x1F7F1,
    },
    Range {
        start: 0x1F800,
        end: 0x1F80C,
    },
    Range {
        start: 0x1F810,
        end: 0x1F848,
    },
    Range {
        start: 0x1F850,
        end: 0x1F85A,
    },
    Range {
        start: 0x1F860,
        end: 0x1F888,
    },
    Range {
        start: 0x1F890,
        end: 0x1F8AE,
    },
    Range {
        start: 0x1F8B0,
        end: 0x1F8BC,
    },
    Range {
        start: 0x1F8C0,
        end: 0x1F8C2,
    },
    Range {
        start: 0x1F900,
        end: 0x1FA54,
    },
    Range {
        start: 0x1FA60,
        end: 0x1FA6E,
    },
    Range {
        start: 0x1FA70,
        end: 0x1FA7D,
    },
    Range {
        start: 0x1FA80,
        end: 0x1FA8A,
    },
    Range {
        start: 0x1FA8F,
        end: 0x1FAC7,
    },
    Range {
        start: 0x1FACE,
        end: 0x1FADD,
    },
    Range {
        start: 0x1FADF,
        end: 0x1FAEA,
    },
    Range {
        start: 0x1FAF0,
        end: 0x1FAF9,
    },
    Range {
        start: 0x1FB00,
        end: 0x1FB93,
    },
    Range {
        start: 0x1FB94,
        end: 0x1FBFA,
    },
    Range {
        start: 0x20000,
        end: 0x2A6E0,
    },
    Range {
        start: 0x2A700,
        end: 0x2B73A,
    },
    Range {
        start: 0x2B740,
        end: 0x2B81E,
    },
    Range {
        start: 0x2B820,
        end: 0x2CEA2,
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
        end: 0x323B0,
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

pub const DECOMPOSITIONS: &[Decomposition] = &[
    Decomposition {
        point: 0xC0,
        offset: 0,
        len: 2,
    },
    Decomposition {
        point: 0xC1,
        offset: 2,
        len: 2,
    },
    Decomposition {
        point: 0xC2,
        offset: 4,
        len: 2,
    },
    Decomposition {
        point: 0xC3,
        offset: 6,
        len: 2,
    },
    Decomposition {
        point: 0xC4,
        offset: 8,
        len: 2,
    },
    Decomposition {
        point: 0xC5,
        offset: 10,
        len: 2,
    },
    Decomposition {
        point: 0xC7,
        offset: 12,
        len: 2,
    },
    Decomposition {
        point: 0xC8,
        offset: 14,
        len: 2,
    },
    Decomposition {
        point: 0xC9,
        offset: 16,
        len: 2,
    },
    Decomposition {
        point: 0xCA,
        offset: 18,
        len: 2,
    },
    Decomposition {
        point: 0xCB,
        offset: 20,
        len: 2,
    },
    Decomposition {
        point: 0xCC,
        offset: 22,
        len: 2,
    },
    Decomposition {
        point: 0xCD,
        offset: 24,
        len: 2,
    },
    Decomposition {
        point: 0xCE,
        offset: 26,
        len: 2,
    },
    Decomposition {
        point: 0xCF,
        offset: 28,
        len: 2,
    },
    Decomposition {
        point: 0xD1,
        offset: 30,
        len: 2,
    },
    Decomposition {
        point: 0xD2,
        offset: 32,
        len: 2,
    },
    Decomposition {
        point: 0xD3,
        offset: 34,
        len: 2,
    },
    Decomposition {
        point: 0xD4,
        offset: 36,
        len: 2,
    },
    Decomposition {
        point: 0xD5,
        offset: 38,
        len: 2,
    },
    Decomposition {
        point: 0xD6,
        offset: 40,
        len: 2,
    },
    Decomposition {
        point: 0xD9,
        offset: 42,
        len: 2,
    },
    Decomposition {
        point: 0xDA,
        offset: 44,
        len: 2,
    },
    Decomposition {
        point: 0xDB,
        offset: 46,
        len: 2,
    },
    Decomposition {
        point: 0xDC,
        offset: 48,
        len: 2,
    },
    Decomposition {
        point: 0xDD,
        offset: 50,
        len: 2,
    },
    Decomposition {
        point: 0xE0,
        offset: 52,
        len: 2,
    },
    Decomposition {
        point: 0xE1,
        offset: 54,
        len: 2,
    },
    Decomposition {
        point: 0xE2,
        offset: 56,
        len: 2,
    },
    Decomposition {
        point: 0xE3,
        offset: 58,
        len: 2,
    },
    Decomposition {
        point: 0xE4,
        offset: 60,
        len: 2,
    },
    Decomposition {
        point: 0xE5,
        offset: 62,
        len: 2,
    },
    Decomposition {
        point: 0xE7,
        offset: 64,
        len: 2,
    },
    Decomposition {
        point: 0xE8,
        offset: 66,
        len: 2,
    },
    Decomposition {
        point: 0xE9,
        offset: 68,
        len: 2,
    },
    Decomposition {
        point: 0xEA,
        offset: 70,
        len: 2,
    },
    Decomposition {
        point: 0xEB,
        offset: 72,
        len: 2,
    },
    Decomposition {
        point: 0xEC,
        offset: 74,
        len: 2,
    },
    Decomposition {
        point: 0xED,
        offset: 76,
        len: 2,
    },
    Decomposition {
        point: 0xEE,
        offset: 78,
        len: 2,
    },
    Decomposition {
        point: 0xEF,
        offset: 80,
        len: 2,
    },
    Decomposition {
        point: 0xF1,
        offset: 82,
        len: 2,
    },
    Decomposition {
        point: 0xF2,
        offset: 84,
        len: 2,
    },
    Decomposition {
        point: 0xF3,
        offset: 86,
        len: 2,
    },
    Decomposition {
        point: 0xF4,
        offset: 88,
        len: 2,
    },
    Decomposition {
        point: 0xF5,
        offset: 90,
        len: 2,
    },
    Decomposition {
        point: 0xF6,
        offset: 92,
        len: 2,
    },
    Decomposition {
        point: 0xF9,
        offset: 94,
        len: 2,
    },
    Decomposition {
        point: 0xFA,
        offset: 96,
        len: 2,
    },
    Decomposition {
        point: 0xFB,
        offset: 98,
        len: 2,
    },
    Decomposition {
        point: 0xFC,
        offset: 100,
        len: 2,
    },
    Decomposition {
        point: 0xFD,
        offset: 102,
        len: 2,
    },
    Decomposition {
        point: 0xFF,
        offset: 104,
        len: 2,
    },
    Decomposition {
        point: 0x100,
        offset: 106,
        len: 2,
    },
    Decomposition {
        point: 0x101,
        offset: 108,
        len: 2,
    },
    Decomposition {
        point: 0x102,
        offset: 110,
        len: 2,
    },
    Decomposition {
        point: 0x103,
        offset: 112,
        len: 2,
    },
    Decomposition {
        point: 0x104,
        offset: 114,
        len: 2,
    },
    Decomposition {
        point: 0x105,
        offset: 116,
        len: 2,
    },
    Decomposition {
        point: 0x106,
        offset: 118,
        len: 2,
    },
    Decomposition {
        point: 0x107,
        offset: 120,
        len: 2,
    },
    Decomposition {
        point: 0x108,
        offset: 122,
        len: 2,
    },
    Decomposition {
        point: 0x109,
        offset: 124,
        len: 2,
    },
    Decomposition {
        point: 0x10A,
        offset: 126,
        len: 2,
    },
    Decomposition {
        point: 0x10B,
        offset: 128,
        len: 2,
    },
    Decomposition {
        point: 0x10C,
        offset: 130,
        len: 2,
    },
    Decomposition {
        point: 0x10D,
        offset: 132,
        len: 2,
    },
    Decomposition {
        point: 0x10E,
        offset: 134,
        len: 2,
    },
    Decomposition {
        point: 0x10F,
        offset: 136,
        len: 2,
    },
    Decomposition {
        point: 0x112,
        offset: 138,
        len: 2,
    },
    Decomposition {
        point: 0x113,
        offset: 140,
        len: 2,
    },
    Decomposition {
        point: 0x114,
        offset: 142,
        len: 2,
    },
    Decomposition {
        point: 0x115,
        offset: 144,
        len: 2,
    },
    Decomposition {
        point: 0x116,
        offset: 146,
        len: 2,
    },
    Decomposition {
        point: 0x117,
        offset: 148,
        len: 2,
    },
    Decomposition {
        point: 0x118,
        offset: 150,
        len: 2,
    },
    Decomposition {
        point: 0x119,
        offset: 152,
        len: 2,
    },
    Decomposition {
        point: 0x11A,
        offset: 154,
        len: 2,
    },
    Decomposition {
        point: 0x11B,
        offset: 156,
        len: 2,
    },
    Decomposition {
        point: 0x11C,
        offset: 158,
        len: 2,
    },
    Decomposition {
        point: 0x11D,
        offset: 160,
        len: 2,
    },
    Decomposition {
        point: 0x11E,
        offset: 162,
        len: 2,
    },
    Decomposition {
        point: 0x11F,
        offset: 164,
        len: 2,
    },
    Decomposition {
        point: 0x120,
        offset: 166,
        len: 2,
    },
    Decomposition {
        point: 0x121,
        offset: 168,
        len: 2,
    },
    Decomposition {
        point: 0x122,
        offset: 170,
        len: 2,
    },
    Decomposition {
        point: 0x123,
        offset: 172,
        len: 2,
    },
    Decomposition {
        point: 0x124,
        offset: 174,
        len: 2,
    },
    Decomposition {
        point: 0x125,
        offset: 176,
        len: 2,
    },
    Decomposition {
        point: 0x128,
        offset: 178,
        len: 2,
    },
    Decomposition {
        point: 0x129,
        offset: 180,
        len: 2,
    },
    Decomposition {
        point: 0x12A,
        offset: 182,
        len: 2,
    },
    Decomposition {
        point: 0x12B,
        offset: 184,
        len: 2,
    },
    Decomposition {
        point: 0x12C,
        offset: 186,
        len: 2,
    },
    Decomposition {
        point: 0x12D,
        offset: 188,
        len: 2,
    },
    Decomposition {
        point: 0x12E,
        offset: 190,
        len: 2,
    },
    Decomposition {
        point: 0x12F,
        offset: 192,
        len: 2,
    },
    Decomposition {
        point: 0x130,
        offset: 194,
        len: 2,
    },
    Decomposition {
        point: 0x134,
        offset: 196,
        len: 2,
    },
    Decomposition {
        point: 0x135,
        offset: 198,
        len: 2,
    },
    Decomposition {
        point: 0x136,
        offset: 200,
        len: 2,
    },
    Decomposition {
        point: 0x137,
        offset: 202,
        len: 2,
    },
    Decomposition {
        point: 0x139,
        offset: 204,
        len: 2,
    },
    Decomposition {
        point: 0x13A,
        offset: 206,
        len: 2,
    },
    Decomposition {
        point: 0x13B,
        offset: 208,
        len: 2,
    },
    Decomposition {
        point: 0x13C,
        offset: 210,
        len: 2,
    },
    Decomposition {
        point: 0x13D,
        offset: 212,
        len: 2,
    },
    Decomposition {
        point: 0x13E,
        offset: 214,
        len: 2,
    },
    Decomposition {
        point: 0x143,
        offset: 216,
        len: 2,
    },
    Decomposition {
        point: 0x144,
        offset: 218,
        len: 2,
    },
    Decomposition {
        point: 0x145,
        offset: 220,
        len: 2,
    },
    Decomposition {
        point: 0x146,
        offset: 222,
        len: 2,
    },
    Decomposition {
        point: 0x147,
        offset: 224,
        len: 2,
    },
    Decomposition {
        point: 0x148,
        offset: 226,
        len: 2,
    },
    Decomposition {
        point: 0x14C,
        offset: 228,
        len: 2,
    },
    Decomposition {
        point: 0x14D,
        offset: 230,
        len: 2,
    },
    Decomposition {
        point: 0x14E,
        offset: 232,
        len: 2,
    },
    Decomposition {
        point: 0x14F,
        offset: 234,
        len: 2,
    },
    Decomposition {
        point: 0x150,
        offset: 236,
        len: 2,
    },
    Decomposition {
        point: 0x151,
        offset: 238,
        len: 2,
    },
    Decomposition {
        point: 0x154,
        offset: 240,
        len: 2,
    },
    Decomposition {
        point: 0x155,
        offset: 242,
        len: 2,
    },
    Decomposition {
        point: 0x156,
        offset: 244,
        len: 2,
    },
    Decomposition {
        point: 0x157,
        offset: 246,
        len: 2,
    },
    Decomposition {
        point: 0x158,
        offset: 248,
        len: 2,
    },
    Decomposition {
        point: 0x159,
        offset: 250,
        len: 2,
    },
    Decomposition {
        point: 0x15A,
        offset: 252,
        len: 2,
    },
    Decomposition {
        point: 0x15B,
        offset: 254,
        len: 2,
    },
    Decomposition {
        point: 0x15C,
        offset: 256,
        len: 2,
    },
    Decomposition {
        point: 0x15D,
        offset: 258,
        len: 2,
    },
    Decomposition {
        point: 0x15E,
        offset: 260,
        len: 2,
    },
    Decomposition {
        point: 0x15F,
        offset: 262,
        len: 2,
    },
    Decomposition {
        point: 0x160,
        offset: 264,
        len: 2,
    },
    Decomposition {
        point: 0x161,
        offset: 266,
        len: 2,
    },
    Decomposition {
        point: 0x162,
        offset: 268,
        len: 2,
    },
    Decomposition {
        point: 0x163,
        offset: 270,
        len: 2,
    },
    Decomposition {
        point: 0x164,
        offset: 272,
        len: 2,
    },
    Decomposition {
        point: 0x165,
        offset: 274,
        len: 2,
    },
    Decomposition {
        point: 0x168,
        offset: 276,
        len: 2,
    },
    Decomposition {
        point: 0x169,
        offset: 278,
        len: 2,
    },
    Decomposition {
        point: 0x16A,
        offset: 280,
        len: 2,
    },
    Decomposition {
        point: 0x16B,
        offset: 282,
        len: 2,
    },
    Decomposition {
        point: 0x16C,
        offset: 284,
        len: 2,
    },
    Decomposition {
        point: 0x16D,
        offset: 286,
        len: 2,
    },
    Decomposition {
        point: 0x16E,
        offset: 288,
        len: 2,
    },
    Decomposition {
        point: 0x16F,
        offset: 290,
        len: 2,
    },
    Decomposition {
        point: 0x170,
        offset: 292,
        len: 2,
    },
    Decomposition {
        point: 0x171,
        offset: 294,
        len: 2,
    },
    Decomposition {
        point: 0x172,
        offset: 296,
        len: 2,
    },
    Decomposition {
        point: 0x173,
        offset: 298,
        len: 2,
    },
    Decomposition {
        point: 0x174,
        offset: 300,
        len: 2,
    },
    Decomposition {
        point: 0x175,
        offset: 302,
        len: 2,
    },
    Decomposition {
        point: 0x176,
        offset: 304,
        len: 2,
    },
    Decomposition {
        point: 0x177,
        offset: 306,
        len: 2,
    },
    Decomposition {
        point: 0x178,
        offset: 308,
        len: 2,
    },
    Decomposition {
        point: 0x179,
        offset: 310,
        len: 2,
    },
    Decomposition {
        point: 0x17A,
        offset: 312,
        len: 2,
    },
    Decomposition {
        point: 0x17B,
        offset: 314,
        len: 2,
    },
    Decomposition {
        point: 0x17C,
        offset: 316,
        len: 2,
    },
    Decomposition {
        point: 0x17D,
        offset: 318,
        len: 2,
    },
    Decomposition {
        point: 0x17E,
        offset: 320,
        len: 2,
    },
    Decomposition {
        point: 0x1A0,
        offset: 322,
        len: 2,
    },
    Decomposition {
        point: 0x1A1,
        offset: 324,
        len: 2,
    },
    Decomposition {
        point: 0x1AF,
        offset: 326,
        len: 2,
    },
    Decomposition {
        point: 0x1B0,
        offset: 328,
        len: 2,
    },
    Decomposition {
        point: 0x1CD,
        offset: 330,
        len: 2,
    },
    Decomposition {
        point: 0x1CE,
        offset: 332,
        len: 2,
    },
    Decomposition {
        point: 0x1CF,
        offset: 334,
        len: 2,
    },
    Decomposition {
        point: 0x1D0,
        offset: 336,
        len: 2,
    },
    Decomposition {
        point: 0x1D1,
        offset: 338,
        len: 2,
    },
    Decomposition {
        point: 0x1D2,
        offset: 340,
        len: 2,
    },
    Decomposition {
        point: 0x1D3,
        offset: 342,
        len: 2,
    },
    Decomposition {
        point: 0x1D4,
        offset: 344,
        len: 2,
    },
    Decomposition {
        point: 0x1D5,
        offset: 346,
        len: 2,
    },
    Decomposition {
        point: 0x1D6,
        offset: 348,
        len: 2,
    },
    Decomposition {
        point: 0x1D7,
        offset: 350,
        len: 2,
    },
    Decomposition {
        point: 0x1D8,
        offset: 352,
        len: 2,
    },
    Decomposition {
        point: 0x1D9,
        offset: 354,
        len: 2,
    },
    Decomposition {
        point: 0x1DA,
        offset: 356,
        len: 2,
    },
    Decomposition {
        point: 0x1DB,
        offset: 358,
        len: 2,
    },
    Decomposition {
        point: 0x1DC,
        offset: 360,
        len: 2,
    },
    Decomposition {
        point: 0x1DE,
        offset: 362,
        len: 2,
    },
    Decomposition {
        point: 0x1DF,
        offset: 364,
        len: 2,
    },
    Decomposition {
        point: 0x1E0,
        offset: 366,
        len: 2,
    },
    Decomposition {
        point: 0x1E1,
        offset: 368,
        len: 2,
    },
    Decomposition {
        point: 0x1E2,
        offset: 370,
        len: 2,
    },
    Decomposition {
        point: 0x1E3,
        offset: 372,
        len: 2,
    },
    Decomposition {
        point: 0x1E6,
        offset: 374,
        len: 2,
    },
    Decomposition {
        point: 0x1E7,
        offset: 376,
        len: 2,
    },
    Decomposition {
        point: 0x1E8,
        offset: 378,
        len: 2,
    },
    Decomposition {
        point: 0x1E9,
        offset: 380,
        len: 2,
    },
    Decomposition {
        point: 0x1EA,
        offset: 382,
        len: 2,
    },
    Decomposition {
        point: 0x1EB,
        offset: 384,
        len: 2,
    },
    Decomposition {
        point: 0x1EC,
        offset: 386,
        len: 2,
    },
    Decomposition {
        point: 0x1ED,
        offset: 388,
        len: 2,
    },
    Decomposition {
        point: 0x1EE,
        offset: 390,
        len: 2,
    },
    Decomposition {
        point: 0x1EF,
        offset: 392,
        len: 2,
    },
    Decomposition {
        point: 0x1F0,
        offset: 394,
        len: 2,
    },
    Decomposition {
        point: 0x1F4,
        offset: 396,
        len: 2,
    },
    Decomposition {
        point: 0x1F5,
        offset: 398,
        len: 2,
    },
    Decomposition {
        point: 0x1F8,
        offset: 400,
        len: 2,
    },
    Decomposition {
        point: 0x1F9,
        offset: 402,
        len: 2,
    },
    Decomposition {
        point: 0x1FA,
        offset: 404,
        len: 2,
    },
    Decomposition {
        point: 0x1FB,
        offset: 406,
        len: 2,
    },
    Decomposition {
        point: 0x1FC,
        offset: 408,
        len: 2,
    },
    Decomposition {
        point: 0x1FD,
        offset: 410,
        len: 2,
    },
    Decomposition {
        point: 0x1FE,
        offset: 412,
        len: 2,
    },
    Decomposition {
        point: 0x1FF,
        offset: 414,
        len: 2,
    },
    Decomposition {
        point: 0x200,
        offset: 416,
        len: 2,
    },
    Decomposition {
        point: 0x201,
        offset: 418,
        len: 2,
    },
    Decomposition {
        point: 0x202,
        offset: 420,
        len: 2,
    },
    Decomposition {
        point: 0x203,
        offset: 422,
        len: 2,
    },
    Decomposition {
        point: 0x204,
        offset: 424,
        len: 2,
    },
    Decomposition {
        point: 0x205,
        offset: 426,
        len: 2,
    },
    Decomposition {
        point: 0x206,
        offset: 428,
        len: 2,
    },
    Decomposition {
        point: 0x207,
        offset: 430,
        len: 2,
    },
    Decomposition {
        point: 0x208,
        offset: 432,
        len: 2,
    },
    Decomposition {
        point: 0x209,
        offset: 434,
        len: 2,
    },
    Decomposition {
        point: 0x20A,
        offset: 436,
        len: 2,
    },
    Decomposition {
        point: 0x20B,
        offset: 438,
        len: 2,
    },
    Decomposition {
        point: 0x20C,
        offset: 440,
        len: 2,
    },
    Decomposition {
        point: 0x20D,
        offset: 442,
        len: 2,
    },
    Decomposition {
        point: 0x20E,
        offset: 444,
        len: 2,
    },
    Decomposition {
        point: 0x20F,
        offset: 446,
        len: 2,
    },
    Decomposition {
        point: 0x210,
        offset: 448,
        len: 2,
    },
    Decomposition {
        point: 0x211,
        offset: 450,
        len: 2,
    },
    Decomposition {
        point: 0x212,
        offset: 452,
        len: 2,
    },
    Decomposition {
        point: 0x213,
        offset: 454,
        len: 2,
    },
    Decomposition {
        point: 0x214,
        offset: 456,
        len: 2,
    },
    Decomposition {
        point: 0x215,
        offset: 458,
        len: 2,
    },
    Decomposition {
        point: 0x216,
        offset: 460,
        len: 2,
    },
    Decomposition {
        point: 0x217,
        offset: 462,
        len: 2,
    },
    Decomposition {
        point: 0x218,
        offset: 464,
        len: 2,
    },
    Decomposition {
        point: 0x219,
        offset: 466,
        len: 2,
    },
    Decomposition {
        point: 0x21A,
        offset: 468,
        len: 2,
    },
    Decomposition {
        point: 0x21B,
        offset: 470,
        len: 2,
    },
    Decomposition {
        point: 0x21E,
        offset: 472,
        len: 2,
    },
    Decomposition {
        point: 0x21F,
        offset: 474,
        len: 2,
    },
    Decomposition {
        point: 0x226,
        offset: 476,
        len: 2,
    },
    Decomposition {
        point: 0x227,
        offset: 478,
        len: 2,
    },
    Decomposition {
        point: 0x228,
        offset: 480,
        len: 2,
    },
    Decomposition {
        point: 0x229,
        offset: 482,
        len: 2,
    },
    Decomposition {
        point: 0x22A,
        offset: 484,
        len: 2,
    },
    Decomposition {
        point: 0x22B,
        offset: 486,
        len: 2,
    },
    Decomposition {
        point: 0x22C,
        offset: 488,
        len: 2,
    },
    Decomposition {
        point: 0x22D,
        offset: 490,
        len: 2,
    },
    Decomposition {
        point: 0x22E,
        offset: 492,
        len: 2,
    },
    Decomposition {
        point: 0x22F,
        offset: 494,
        len: 2,
    },
    Decomposition {
        point: 0x230,
        offset: 496,
        len: 2,
    },
    Decomposition {
        point: 0x231,
        offset: 498,
        len: 2,
    },
    Decomposition {
        point: 0x232,
        offset: 500,
        len: 2,
    },
    Decomposition {
        point: 0x233,
        offset: 502,
        len: 2,
    },
    Decomposition {
        point: 0x340,
        offset: 504,
        len: 1,
    },
    Decomposition {
        point: 0x341,
        offset: 505,
        len: 1,
    },
    Decomposition {
        point: 0x343,
        offset: 506,
        len: 1,
    },
    Decomposition {
        point: 0x344,
        offset: 507,
        len: 2,
    },
    Decomposition {
        point: 0x374,
        offset: 509,
        len: 1,
    },
    Decomposition {
        point: 0x37E,
        offset: 510,
        len: 1,
    },
    Decomposition {
        point: 0x385,
        offset: 511,
        len: 2,
    },
    Decomposition {
        point: 0x386,
        offset: 513,
        len: 2,
    },
    Decomposition {
        point: 0x387,
        offset: 515,
        len: 1,
    },
    Decomposition {
        point: 0x388,
        offset: 516,
        len: 2,
    },
    Decomposition {
        point: 0x389,
        offset: 518,
        len: 2,
    },
    Decomposition {
        point: 0x38A,
        offset: 520,
        len: 2,
    },
    Decomposition {
        point: 0x38C,
        offset: 522,
        len: 2,
    },
    Decomposition {
        point: 0x38E,
        offset: 524,
        len: 2,
    },
    Decomposition {
        point: 0x38F,
        offset: 526,
        len: 2,
    },
    Decomposition {
        point: 0x390,
        offset: 528,
        len: 2,
    },
    Decomposition {
        point: 0x3AA,
        offset: 530,
        len: 2,
    },
    Decomposition {
        point: 0x3AB,
        offset: 532,
        len: 2,
    },
    Decomposition {
        point: 0x3AC,
        offset: 534,
        len: 2,
    },
    Decomposition {
        point: 0x3AD,
        offset: 536,
        len: 2,
    },
    Decomposition {
        point: 0x3AE,
        offset: 538,
        len: 2,
    },
    Decomposition {
        point: 0x3AF,
        offset: 540,
        len: 2,
    },
    Decomposition {
        point: 0x3B0,
        offset: 542,
        len: 2,
    },
    Decomposition {
        point: 0x3CA,
        offset: 544,
        len: 2,
    },
    Decomposition {
        point: 0x3CB,
        offset: 546,
        len: 2,
    },
    Decomposition {
        point: 0x3CC,
        offset: 548,
        len: 2,
    },
    Decomposition {
        point: 0x3CD,
        offset: 550,
        len: 2,
    },
    Decomposition {
        point: 0x3CE,
        offset: 552,
        len: 2,
    },
    Decomposition {
        point: 0x3D3,
        offset: 554,
        len: 2,
    },
    Decomposition {
        point: 0x3D4,
        offset: 556,
        len: 2,
    },
    Decomposition {
        point: 0x400,
        offset: 558,
        len: 2,
    },
    Decomposition {
        point: 0x401,
        offset: 560,
        len: 2,
    },
    Decomposition {
        point: 0x403,
        offset: 562,
        len: 2,
    },
    Decomposition {
        point: 0x407,
        offset: 564,
        len: 2,
    },
    Decomposition {
        point: 0x40C,
        offset: 566,
        len: 2,
    },
    Decomposition {
        point: 0x40D,
        offset: 568,
        len: 2,
    },
    Decomposition {
        point: 0x40E,
        offset: 570,
        len: 2,
    },
    Decomposition {
        point: 0x419,
        offset: 572,
        len: 2,
    },
    Decomposition {
        point: 0x439,
        offset: 574,
        len: 2,
    },
    Decomposition {
        point: 0x450,
        offset: 576,
        len: 2,
    },
    Decomposition {
        point: 0x451,
        offset: 578,
        len: 2,
    },
    Decomposition {
        point: 0x453,
        offset: 580,
        len: 2,
    },
    Decomposition {
        point: 0x457,
        offset: 582,
        len: 2,
    },
    Decomposition {
        point: 0x45C,
        offset: 584,
        len: 2,
    },
    Decomposition {
        point: 0x45D,
        offset: 586,
        len: 2,
    },
    Decomposition {
        point: 0x45E,
        offset: 588,
        len: 2,
    },
    Decomposition {
        point: 0x476,
        offset: 590,
        len: 2,
    },
    Decomposition {
        point: 0x477,
        offset: 592,
        len: 2,
    },
    Decomposition {
        point: 0x4C1,
        offset: 594,
        len: 2,
    },
    Decomposition {
        point: 0x4C2,
        offset: 596,
        len: 2,
    },
    Decomposition {
        point: 0x4D0,
        offset: 598,
        len: 2,
    },
    Decomposition {
        point: 0x4D1,
        offset: 600,
        len: 2,
    },
    Decomposition {
        point: 0x4D2,
        offset: 602,
        len: 2,
    },
    Decomposition {
        point: 0x4D3,
        offset: 604,
        len: 2,
    },
    Decomposition {
        point: 0x4D6,
        offset: 606,
        len: 2,
    },
    Decomposition {
        point: 0x4D7,
        offset: 608,
        len: 2,
    },
    Decomposition {
        point: 0x4DA,
        offset: 610,
        len: 2,
    },
    Decomposition {
        point: 0x4DB,
        offset: 612,
        len: 2,
    },
    Decomposition {
        point: 0x4DC,
        offset: 614,
        len: 2,
    },
    Decomposition {
        point: 0x4DD,
        offset: 616,
        len: 2,
    },
    Decomposition {
        point: 0x4DE,
        offset: 618,
        len: 2,
    },
    Decomposition {
        point: 0x4DF,
        offset: 620,
        len: 2,
    },
    Decomposition {
        point: 0x4E2,
        offset: 622,
        len: 2,
    },
    Decomposition {
        point: 0x4E3,
        offset: 624,
        len: 2,
    },
    Decomposition {
        point: 0x4E4,
        offset: 626,
        len: 2,
    },
    Decomposition {
        point: 0x4E5,
        offset: 628,
        len: 2,
    },
    Decomposition {
        point: 0x4E6,
        offset: 630,
        len: 2,
    },
    Decomposition {
        point: 0x4E7,
        offset: 632,
        len: 2,
    },
    Decomposition {
        point: 0x4EA,
        offset: 634,
        len: 2,
    },
    Decomposition {
        point: 0x4EB,
        offset: 636,
        len: 2,
    },
    Decomposition {
        point: 0x4EC,
        offset: 638,
        len: 2,
    },
    Decomposition {
        point: 0x4ED,
        offset: 640,
        len: 2,
    },
    Decomposition {
        point: 0x4EE,
        offset: 642,
        len: 2,
    },
    Decomposition {
        point: 0x4EF,
        offset: 644,
        len: 2,
    },
    Decomposition {
        point: 0x4F0,
        offset: 646,
        len: 2,
    },
    Decomposition {
        point: 0x4F1,
        offset: 648,
        len: 2,
    },
    Decomposition {
        point: 0x4F2,
        offset: 650,
        len: 2,
    },
    Decomposition {
        point: 0x4F3,
        offset: 652,
        len: 2,
    },
    Decomposition {
        point: 0x4F4,
        offset: 654,
        len: 2,
    },
    Decomposition {
        point: 0x4F5,
        offset: 656,
        len: 2,
    },
    Decomposition {
        point: 0x4F8,
        offset: 658,
        len: 2,
    },
    Decomposition {
        point: 0x4F9,
        offset: 660,
        len: 2,
    },
    Decomposition {
        point: 0x622,
        offset: 662,
        len: 2,
    },
    Decomposition {
        point: 0x623,
        offset: 664,
        len: 2,
    },
    Decomposition {
        point: 0x624,
        offset: 666,
        len: 2,
    },
    Decomposition {
        point: 0x625,
        offset: 668,
        len: 2,
    },
    Decomposition {
        point: 0x626,
        offset: 670,
        len: 2,
    },
    Decomposition {
        point: 0x6C0,
        offset: 672,
        len: 2,
    },
    Decomposition {
        point: 0x6C2,
        offset: 674,
        len: 2,
    },
    Decomposition {
        point: 0x6D3,
        offset: 676,
        len: 2,
    },
    Decomposition {
        point: 0x929,
        offset: 678,
        len: 2,
    },
    Decomposition {
        point: 0x931,
        offset: 680,
        len: 2,
    },
    Decomposition {
        point: 0x934,
        offset: 682,
        len: 2,
    },
    Decomposition {
        point: 0x958,
        offset: 684,
        len: 2,
    },
    Decomposition {
        point: 0x959,
        offset: 686,
        len: 2,
    },
    Decomposition {
        point: 0x95A,
        offset: 688,
        len: 2,
    },
    Decomposition {
        point: 0x95B,
        offset: 690,
        len: 2,
    },
    Decomposition {
        point: 0x95C,
        offset: 692,
        len: 2,
    },
    Decomposition {
        point: 0x95D,
        offset: 694,
        len: 2,
    },
    Decomposition {
        point: 0x95E,
        offset: 696,
        len: 2,
    },
    Decomposition {
        point: 0x95F,
        offset: 698,
        len: 2,
    },
    Decomposition {
        point: 0x9CB,
        offset: 700,
        len: 2,
    },
    Decomposition {
        point: 0x9CC,
        offset: 702,
        len: 2,
    },
    Decomposition {
        point: 0x9DC,
        offset: 704,
        len: 2,
    },
    Decomposition {
        point: 0x9DD,
        offset: 706,
        len: 2,
    },
    Decomposition {
        point: 0x9DF,
        offset: 708,
        len: 2,
    },
    Decomposition {
        point: 0xA33,
        offset: 710,
        len: 2,
    },
    Decomposition {
        point: 0xA36,
        offset: 712,
        len: 2,
    },
    Decomposition {
        point: 0xA59,
        offset: 714,
        len: 2,
    },
    Decomposition {
        point: 0xA5A,
        offset: 716,
        len: 2,
    },
    Decomposition {
        point: 0xA5B,
        offset: 718,
        len: 2,
    },
    Decomposition {
        point: 0xA5E,
        offset: 720,
        len: 2,
    },
    Decomposition {
        point: 0xB48,
        offset: 722,
        len: 2,
    },
    Decomposition {
        point: 0xB4B,
        offset: 724,
        len: 2,
    },
    Decomposition {
        point: 0xB4C,
        offset: 726,
        len: 2,
    },
    Decomposition {
        point: 0xB5C,
        offset: 728,
        len: 2,
    },
    Decomposition {
        point: 0xB5D,
        offset: 730,
        len: 2,
    },
    Decomposition {
        point: 0xB94,
        offset: 732,
        len: 2,
    },
    Decomposition {
        point: 0xBCA,
        offset: 734,
        len: 2,
    },
    Decomposition {
        point: 0xBCB,
        offset: 736,
        len: 2,
    },
    Decomposition {
        point: 0xBCC,
        offset: 738,
        len: 2,
    },
    Decomposition {
        point: 0xC48,
        offset: 740,
        len: 2,
    },
    Decomposition {
        point: 0xCC0,
        offset: 742,
        len: 2,
    },
    Decomposition {
        point: 0xCC7,
        offset: 744,
        len: 2,
    },
    Decomposition {
        point: 0xCC8,
        offset: 746,
        len: 2,
    },
    Decomposition {
        point: 0xCCA,
        offset: 748,
        len: 2,
    },
    Decomposition {
        point: 0xCCB,
        offset: 750,
        len: 2,
    },
    Decomposition {
        point: 0xD4A,
        offset: 752,
        len: 2,
    },
    Decomposition {
        point: 0xD4B,
        offset: 754,
        len: 2,
    },
    Decomposition {
        point: 0xD4C,
        offset: 756,
        len: 2,
    },
    Decomposition {
        point: 0xDDA,
        offset: 758,
        len: 2,
    },
    Decomposition {
        point: 0xDDC,
        offset: 760,
        len: 2,
    },
    Decomposition {
        point: 0xDDD,
        offset: 762,
        len: 2,
    },
    Decomposition {
        point: 0xDDE,
        offset: 764,
        len: 2,
    },
    Decomposition {
        point: 0xF43,
        offset: 766,
        len: 2,
    },
    Decomposition {
        point: 0xF4D,
        offset: 768,
        len: 2,
    },
    Decomposition {
        point: 0xF52,
        offset: 770,
        len: 2,
    },
    Decomposition {
        point: 0xF57,
        offset: 772,
        len: 2,
    },
    Decomposition {
        point: 0xF5C,
        offset: 774,
        len: 2,
    },
    Decomposition {
        point: 0xF69,
        offset: 776,
        len: 2,
    },
    Decomposition {
        point: 0xF73,
        offset: 778,
        len: 2,
    },
    Decomposition {
        point: 0xF75,
        offset: 780,
        len: 2,
    },
    Decomposition {
        point: 0xF76,
        offset: 782,
        len: 2,
    },
    Decomposition {
        point: 0xF78,
        offset: 784,
        len: 2,
    },
    Decomposition {
        point: 0xF81,
        offset: 786,
        len: 2,
    },
    Decomposition {
        point: 0xF93,
        offset: 788,
        len: 2,
    },
    Decomposition {
        point: 0xF9D,
        offset: 790,
        len: 2,
    },
    Decomposition {
        point: 0xFA2,
        offset: 792,
        len: 2,
    },
    Decomposition {
        point: 0xFA7,
        offset: 794,
        len: 2,
    },
    Decomposition {
        point: 0xFAC,
        offset: 796,
        len: 2,
    },
    Decomposition {
        point: 0xFB9,
        offset: 798,
        len: 2,
    },
    Decomposition {
        point: 0x1026,
        offset: 800,
        len: 2,
    },
    Decomposition {
        point: 0x1B06,
        offset: 802,
        len: 2,
    },
    Decomposition {
        point: 0x1B08,
        offset: 804,
        len: 2,
    },
    Decomposition {
        point: 0x1B0A,
        offset: 806,
        len: 2,
    },
    Decomposition {
        point: 0x1B0C,
        offset: 808,
        len: 2,
    },
    Decomposition {
        point: 0x1B0E,
        offset: 810,
        len: 2,
    },
    Decomposition {
        point: 0x1B12,
        offset: 812,
        len: 2,
    },
    Decomposition {
        point: 0x1B3B,
        offset: 814,
        len: 2,
    },
    Decomposition {
        point: 0x1B3D,
        offset: 816,
        len: 2,
    },
    Decomposition {
        point: 0x1B40,
        offset: 818,
        len: 2,
    },
    Decomposition {
        point: 0x1B41,
        offset: 820,
        len: 2,
    },
    Decomposition {
        point: 0x1B43,
        offset: 822,
        len: 2,
    },
    Decomposition {
        point: 0x1E00,
        offset: 824,
        len: 2,
    },
    Decomposition {
        point: 0x1E01,
        offset: 826,
        len: 2,
    },
    Decomposition {
        point: 0x1E02,
        offset: 828,
        len: 2,
    },
    Decomposition {
        point: 0x1E03,
        offset: 830,
        len: 2,
    },
    Decomposition {
        point: 0x1E04,
        offset: 832,
        len: 2,
    },
    Decomposition {
        point: 0x1E05,
        offset: 834,
        len: 2,
    },
    Decomposition {
        point: 0x1E06,
        offset: 836,
        len: 2,
    },
    Decomposition {
        point: 0x1E07,
        offset: 838,
        len: 2,
    },
    Decomposition {
        point: 0x1E08,
        offset: 840,
        len: 2,
    },
    Decomposition {
        point: 0x1E09,
        offset: 842,
        len: 2,
    },
    Decomposition {
        point: 0x1E0A,
        offset: 844,
        len: 2,
    },
    Decomposition {
        point: 0x1E0B,
        offset: 846,
        len: 2,
    },
    Decomposition {
        point: 0x1E0C,
        offset: 848,
        len: 2,
    },
    Decomposition {
        point: 0x1E0D,
        offset: 850,
        len: 2,
    },
    Decomposition {
        point: 0x1E0E,
        offset: 852,
        len: 2,
    },
    Decomposition {
        point: 0x1E0F,
        offset: 854,
        len: 2,
    },
    Decomposition {
        point: 0x1E10,
        offset: 856,
        len: 2,
    },
    Decomposition {
        point: 0x1E11,
        offset: 858,
        len: 2,
    },
    Decomposition {
        point: 0x1E12,
        offset: 860,
        len: 2,
    },
    Decomposition {
        point: 0x1E13,
        offset: 862,
        len: 2,
    },
    Decomposition {
        point: 0x1E14,
        offset: 864,
        len: 2,
    },
    Decomposition {
        point: 0x1E15,
        offset: 866,
        len: 2,
    },
    Decomposition {
        point: 0x1E16,
        offset: 868,
        len: 2,
    },
    Decomposition {
        point: 0x1E17,
        offset: 870,
        len: 2,
    },
    Decomposition {
        point: 0x1E18,
        offset: 872,
        len: 2,
    },
    Decomposition {
        point: 0x1E19,
        offset: 874,
        len: 2,
    },
    Decomposition {
        point: 0x1E1A,
        offset: 876,
        len: 2,
    },
    Decomposition {
        point: 0x1E1B,
        offset: 878,
        len: 2,
    },
    Decomposition {
        point: 0x1E1C,
        offset: 880,
        len: 2,
    },
    Decomposition {
        point: 0x1E1D,
        offset: 882,
        len: 2,
    },
    Decomposition {
        point: 0x1E1E,
        offset: 884,
        len: 2,
    },
    Decomposition {
        point: 0x1E1F,
        offset: 886,
        len: 2,
    },
    Decomposition {
        point: 0x1E20,
        offset: 888,
        len: 2,
    },
    Decomposition {
        point: 0x1E21,
        offset: 890,
        len: 2,
    },
    Decomposition {
        point: 0x1E22,
        offset: 892,
        len: 2,
    },
    Decomposition {
        point: 0x1E23,
        offset: 894,
        len: 2,
    },
    Decomposition {
        point: 0x1E24,
        offset: 896,
        len: 2,
    },
    Decomposition {
        point: 0x1E25,
        offset: 898,
        len: 2,
    },
    Decomposition {
        point: 0x1E26,
        offset: 900,
        len: 2,
    },
    Decomposition {
        point: 0x1E27,
        offset: 902,
        len: 2,
    },
    Decomposition {
        point: 0x1E28,
        offset: 904,
        len: 2,
    },
    Decomposition {
        point: 0x1E29,
        offset: 906,
        len: 2,
    },
    Decomposition {
        point: 0x1E2A,
        offset: 908,
        len: 2,
    },
    Decomposition {
        point: 0x1E2B,
        offset: 910,
        len: 2,
    },
    Decomposition {
        point: 0x1E2C,
        offset: 912,
        len: 2,
    },
    Decomposition {
        point: 0x1E2D,
        offset: 914,
        len: 2,
    },
    Decomposition {
        point: 0x1E2E,
        offset: 916,
        len: 2,
    },
    Decomposition {
        point: 0x1E2F,
        offset: 918,
        len: 2,
    },
    Decomposition {
        point: 0x1E30,
        offset: 920,
        len: 2,
    },
    Decomposition {
        point: 0x1E31,
        offset: 922,
        len: 2,
    },
    Decomposition {
        point: 0x1E32,
        offset: 924,
        len: 2,
    },
    Decomposition {
        point: 0x1E33,
        offset: 926,
        len: 2,
    },
    Decomposition {
        point: 0x1E34,
        offset: 928,
        len: 2,
    },
    Decomposition {
        point: 0x1E35,
        offset: 930,
        len: 2,
    },
    Decomposition {
        point: 0x1E36,
        offset: 932,
        len: 2,
    },
    Decomposition {
        point: 0x1E37,
        offset: 934,
        len: 2,
    },
    Decomposition {
        point: 0x1E38,
        offset: 936,
        len: 2,
    },
    Decomposition {
        point: 0x1E39,
        offset: 938,
        len: 2,
    },
    Decomposition {
        point: 0x1E3A,
        offset: 940,
        len: 2,
    },
    Decomposition {
        point: 0x1E3B,
        offset: 942,
        len: 2,
    },
    Decomposition {
        point: 0x1E3C,
        offset: 944,
        len: 2,
    },
    Decomposition {
        point: 0x1E3D,
        offset: 946,
        len: 2,
    },
    Decomposition {
        point: 0x1E3E,
        offset: 948,
        len: 2,
    },
    Decomposition {
        point: 0x1E3F,
        offset: 950,
        len: 2,
    },
    Decomposition {
        point: 0x1E40,
        offset: 952,
        len: 2,
    },
    Decomposition {
        point: 0x1E41,
        offset: 954,
        len: 2,
    },
    Decomposition {
        point: 0x1E42,
        offset: 956,
        len: 2,
    },
    Decomposition {
        point: 0x1E43,
        offset: 958,
        len: 2,
    },
    Decomposition {
        point: 0x1E44,
        offset: 960,
        len: 2,
    },
    Decomposition {
        point: 0x1E45,
        offset: 962,
        len: 2,
    },
    Decomposition {
        point: 0x1E46,
        offset: 964,
        len: 2,
    },
    Decomposition {
        point: 0x1E47,
        offset: 966,
        len: 2,
    },
    Decomposition {
        point: 0x1E48,
        offset: 968,
        len: 2,
    },
    Decomposition {
        point: 0x1E49,
        offset: 970,
        len: 2,
    },
    Decomposition {
        point: 0x1E4A,
        offset: 972,
        len: 2,
    },
    Decomposition {
        point: 0x1E4B,
        offset: 974,
        len: 2,
    },
    Decomposition {
        point: 0x1E4C,
        offset: 976,
        len: 2,
    },
    Decomposition {
        point: 0x1E4D,
        offset: 978,
        len: 2,
    },
    Decomposition {
        point: 0x1E4E,
        offset: 980,
        len: 2,
    },
    Decomposition {
        point: 0x1E4F,
        offset: 982,
        len: 2,
    },
    Decomposition {
        point: 0x1E50,
        offset: 984,
        len: 2,
    },
    Decomposition {
        point: 0x1E51,
        offset: 986,
        len: 2,
    },
    Decomposition {
        point: 0x1E52,
        offset: 988,
        len: 2,
    },
    Decomposition {
        point: 0x1E53,
        offset: 990,
        len: 2,
    },
    Decomposition {
        point: 0x1E54,
        offset: 992,
        len: 2,
    },
    Decomposition {
        point: 0x1E55,
        offset: 994,
        len: 2,
    },
    Decomposition {
        point: 0x1E56,
        offset: 996,
        len: 2,
    },
    Decomposition {
        point: 0x1E57,
        offset: 998,
        len: 2,
    },
    Decomposition {
        point: 0x1E58,
        offset: 1000,
        len: 2,
    },
    Decomposition {
        point: 0x1E59,
        offset: 1002,
        len: 2,
    },
    Decomposition {
        point: 0x1E5A,
        offset: 1004,
        len: 2,
    },
    Decomposition {
        point: 0x1E5B,
        offset: 1006,
        len: 2,
    },
    Decomposition {
        point: 0x1E5C,
        offset: 1008,
        len: 2,
    },
    Decomposition {
        point: 0x1E5D,
        offset: 1010,
        len: 2,
    },
    Decomposition {
        point: 0x1E5E,
        offset: 1012,
        len: 2,
    },
    Decomposition {
        point: 0x1E5F,
        offset: 1014,
        len: 2,
    },
    Decomposition {
        point: 0x1E60,
        offset: 1016,
        len: 2,
    },
    Decomposition {
        point: 0x1E61,
        offset: 1018,
        len: 2,
    },
    Decomposition {
        point: 0x1E62,
        offset: 1020,
        len: 2,
    },
    Decomposition {
        point: 0x1E63,
        offset: 1022,
        len: 2,
    },
    Decomposition {
        point: 0x1E64,
        offset: 1024,
        len: 2,
    },
    Decomposition {
        point: 0x1E65,
        offset: 1026,
        len: 2,
    },
    Decomposition {
        point: 0x1E66,
        offset: 1028,
        len: 2,
    },
    Decomposition {
        point: 0x1E67,
        offset: 1030,
        len: 2,
    },
    Decomposition {
        point: 0x1E68,
        offset: 1032,
        len: 2,
    },
    Decomposition {
        point: 0x1E69,
        offset: 1034,
        len: 2,
    },
    Decomposition {
        point: 0x1E6A,
        offset: 1036,
        len: 2,
    },
    Decomposition {
        point: 0x1E6B,
        offset: 1038,
        len: 2,
    },
    Decomposition {
        point: 0x1E6C,
        offset: 1040,
        len: 2,
    },
    Decomposition {
        point: 0x1E6D,
        offset: 1042,
        len: 2,
    },
    Decomposition {
        point: 0x1E6E,
        offset: 1044,
        len: 2,
    },
    Decomposition {
        point: 0x1E6F,
        offset: 1046,
        len: 2,
    },
    Decomposition {
        point: 0x1E70,
        offset: 1048,
        len: 2,
    },
    Decomposition {
        point: 0x1E71,
        offset: 1050,
        len: 2,
    },
    Decomposition {
        point: 0x1E72,
        offset: 1052,
        len: 2,
    },
    Decomposition {
        point: 0x1E73,
        offset: 1054,
        len: 2,
    },
    Decomposition {
        point: 0x1E74,
        offset: 1056,
        len: 2,
    },
    Decomposition {
        point: 0x1E75,
        offset: 1058,
        len: 2,
    },
    Decomposition {
        point: 0x1E76,
        offset: 1060,
        len: 2,
    },
    Decomposition {
        point: 0x1E77,
        offset: 1062,
        len: 2,
    },
    Decomposition {
        point: 0x1E78,
        offset: 1064,
        len: 2,
    },
    Decomposition {
        point: 0x1E79,
        offset: 1066,
        len: 2,
    },
    Decomposition {
        point: 0x1E7A,
        offset: 1068,
        len: 2,
    },
    Decomposition {
        point: 0x1E7B,
        offset: 1070,
        len: 2,
    },
    Decomposition {
        point: 0x1E7C,
        offset: 1072,
        len: 2,
    },
    Decomposition {
        point: 0x1E7D,
        offset: 1074,
        len: 2,
    },
    Decomposition {
        point: 0x1E7E,
        offset: 1076,
        len: 2,
    },
    Decomposition {
        point: 0x1E7F,
        offset: 1078,
        len: 2,
    },
    Decomposition {
        point: 0x1E80,
        offset: 1080,
        len: 2,
    },
    Decomposition {
        point: 0x1E81,
        offset: 1082,
        len: 2,
    },
    Decomposition {
        point: 0x1E82,
        offset: 1084,
        len: 2,
    },
    Decomposition {
        point: 0x1E83,
        offset: 1086,
        len: 2,
    },
    Decomposition {
        point: 0x1E84,
        offset: 1088,
        len: 2,
    },
    Decomposition {
        point: 0x1E85,
        offset: 1090,
        len: 2,
    },
    Decomposition {
        point: 0x1E86,
        offset: 1092,
        len: 2,
    },
    Decomposition {
        point: 0x1E87,
        offset: 1094,
        len: 2,
    },
    Decomposition {
        point: 0x1E88,
        offset: 1096,
        len: 2,
    },
    Decomposition {
        point: 0x1E89,
        offset: 1098,
        len: 2,
    },
    Decomposition {
        point: 0x1E8A,
        offset: 1100,
        len: 2,
    },
    Decomposition {
        point: 0x1E8B,
        offset: 1102,
        len: 2,
    },
    Decomposition {
        point: 0x1E8C,
        offset: 1104,
        len: 2,
    },
    Decomposition {
        point: 0x1E8D,
        offset: 1106,
        len: 2,
    },
    Decomposition {
        point: 0x1E8E,
        offset: 1108,
        len: 2,
    },
    Decomposition {
        point: 0x1E8F,
        offset: 1110,
        len: 2,
    },
    Decomposition {
        point: 0x1E90,
        offset: 1112,
        len: 2,
    },
    Decomposition {
        point: 0x1E91,
        offset: 1114,
        len: 2,
    },
    Decomposition {
        point: 0x1E92,
        offset: 1116,
        len: 2,
    },
    Decomposition {
        point: 0x1E93,
        offset: 1118,
        len: 2,
    },
    Decomposition {
        point: 0x1E94,
        offset: 1120,
        len: 2,
    },
    Decomposition {
        point: 0x1E95,
        offset: 1122,
        len: 2,
    },
    Decomposition {
        point: 0x1E96,
        offset: 1124,
        len: 2,
    },
    Decomposition {
        point: 0x1E97,
        offset: 1126,
        len: 2,
    },
    Decomposition {
        point: 0x1E98,
        offset: 1128,
        len: 2,
    },
    Decomposition {
        point: 0x1E99,
        offset: 1130,
        len: 2,
    },
    Decomposition {
        point: 0x1E9B,
        offset: 1132,
        len: 2,
    },
    Decomposition {
        point: 0x1EA0,
        offset: 1134,
        len: 2,
    },
    Decomposition {
        point: 0x1EA1,
        offset: 1136,
        len: 2,
    },
    Decomposition {
        point: 0x1EA2,
        offset: 1138,
        len: 2,
    },
    Decomposition {
        point: 0x1EA3,
        offset: 1140,
        len: 2,
    },
    Decomposition {
        point: 0x1EA4,
        offset: 1142,
        len: 2,
    },
    Decomposition {
        point: 0x1EA5,
        offset: 1144,
        len: 2,
    },
    Decomposition {
        point: 0x1EA6,
        offset: 1146,
        len: 2,
    },
    Decomposition {
        point: 0x1EA7,
        offset: 1148,
        len: 2,
    },
    Decomposition {
        point: 0x1EA8,
        offset: 1150,
        len: 2,
    },
    Decomposition {
        point: 0x1EA9,
        offset: 1152,
        len: 2,
    },
    Decomposition {
        point: 0x1EAA,
        offset: 1154,
        len: 2,
    },
    Decomposition {
        point: 0x1EAB,
        offset: 1156,
        len: 2,
    },
    Decomposition {
        point: 0x1EAC,
        offset: 1158,
        len: 2,
    },
    Decomposition {
        point: 0x1EAD,
        offset: 1160,
        len: 2,
    },
    Decomposition {
        point: 0x1EAE,
        offset: 1162,
        len: 2,
    },
    Decomposition {
        point: 0x1EAF,
        offset: 1164,
        len: 2,
    },
    Decomposition {
        point: 0x1EB0,
        offset: 1166,
        len: 2,
    },
    Decomposition {
        point: 0x1EB1,
        offset: 1168,
        len: 2,
    },
    Decomposition {
        point: 0x1EB2,
        offset: 1170,
        len: 2,
    },
    Decomposition {
        point: 0x1EB3,
        offset: 1172,
        len: 2,
    },
    Decomposition {
        point: 0x1EB4,
        offset: 1174,
        len: 2,
    },
    Decomposition {
        point: 0x1EB5,
        offset: 1176,
        len: 2,
    },
    Decomposition {
        point: 0x1EB6,
        offset: 1178,
        len: 2,
    },
    Decomposition {
        point: 0x1EB7,
        offset: 1180,
        len: 2,
    },
    Decomposition {
        point: 0x1EB8,
        offset: 1182,
        len: 2,
    },
    Decomposition {
        point: 0x1EB9,
        offset: 1184,
        len: 2,
    },
    Decomposition {
        point: 0x1EBA,
        offset: 1186,
        len: 2,
    },
    Decomposition {
        point: 0x1EBB,
        offset: 1188,
        len: 2,
    },
    Decomposition {
        point: 0x1EBC,
        offset: 1190,
        len: 2,
    },
    Decomposition {
        point: 0x1EBD,
        offset: 1192,
        len: 2,
    },
    Decomposition {
        point: 0x1EBE,
        offset: 1194,
        len: 2,
    },
    Decomposition {
        point: 0x1EBF,
        offset: 1196,
        len: 2,
    },
    Decomposition {
        point: 0x1EC0,
        offset: 1198,
        len: 2,
    },
    Decomposition {
        point: 0x1EC1,
        offset: 1200,
        len: 2,
    },
    Decomposition {
        point: 0x1EC2,
        offset: 1202,
        len: 2,
    },
    Decomposition {
        point: 0x1EC3,
        offset: 1204,
        len: 2,
    },
    Decomposition {
        point: 0x1EC4,
        offset: 1206,
        len: 2,
    },
    Decomposition {
        point: 0x1EC5,
        offset: 1208,
        len: 2,
    },
    Decomposition {
        point: 0x1EC6,
        offset: 1210,
        len: 2,
    },
    Decomposition {
        point: 0x1EC7,
        offset: 1212,
        len: 2,
    },
    Decomposition {
        point: 0x1EC8,
        offset: 1214,
        len: 2,
    },
    Decomposition {
        point: 0x1EC9,
        offset: 1216,
        len: 2,
    },
    Decomposition {
        point: 0x1ECA,
        offset: 1218,
        len: 2,
    },
    Decomposition {
        point: 0x1ECB,
        offset: 1220,
        len: 2,
    },
    Decomposition {
        point: 0x1ECC,
        offset: 1222,
        len: 2,
    },
    Decomposition {
        point: 0x1ECD,
        offset: 1224,
        len: 2,
    },
    Decomposition {
        point: 0x1ECE,
        offset: 1226,
        len: 2,
    },
    Decomposition {
        point: 0x1ECF,
        offset: 1228,
        len: 2,
    },
    Decomposition {
        point: 0x1ED0,
        offset: 1230,
        len: 2,
    },
    Decomposition {
        point: 0x1ED1,
        offset: 1232,
        len: 2,
    },
    Decomposition {
        point: 0x1ED2,
        offset: 1234,
        len: 2,
    },
    Decomposition {
        point: 0x1ED3,
        offset: 1236,
        len: 2,
    },
    Decomposition {
        point: 0x1ED4,
        offset: 1238,
        len: 2,
    },
    Decomposition {
        point: 0x1ED5,
        offset: 1240,
        len: 2,
    },
    Decomposition {
        point: 0x1ED6,
        offset: 1242,
        len: 2,
    },
    Decomposition {
        point: 0x1ED7,
        offset: 1244,
        len: 2,
    },
    Decomposition {
        point: 0x1ED8,
        offset: 1246,
        len: 2,
    },
    Decomposition {
        point: 0x1ED9,
        offset: 1248,
        len: 2,
    },
    Decomposition {
        point: 0x1EDA,
        offset: 1250,
        len: 2,
    },
    Decomposition {
        point: 0x1EDB,
        offset: 1252,
        len: 2,
    },
    Decomposition {
        point: 0x1EDC,
        offset: 1254,
        len: 2,
    },
    Decomposition {
        point: 0x1EDD,
        offset: 1256,
        len: 2,
    },
    Decomposition {
        point: 0x1EDE,
        offset: 1258,
        len: 2,
    },
    Decomposition {
        point: 0x1EDF,
        offset: 1260,
        len: 2,
    },
    Decomposition {
        point: 0x1EE0,
        offset: 1262,
        len: 2,
    },
    Decomposition {
        point: 0x1EE1,
        offset: 1264,
        len: 2,
    },
    Decomposition {
        point: 0x1EE2,
        offset: 1266,
        len: 2,
    },
    Decomposition {
        point: 0x1EE3,
        offset: 1268,
        len: 2,
    },
    Decomposition {
        point: 0x1EE4,
        offset: 1270,
        len: 2,
    },
    Decomposition {
        point: 0x1EE5,
        offset: 1272,
        len: 2,
    },
    Decomposition {
        point: 0x1EE6,
        offset: 1274,
        len: 2,
    },
    Decomposition {
        point: 0x1EE7,
        offset: 1276,
        len: 2,
    },
    Decomposition {
        point: 0x1EE8,
        offset: 1278,
        len: 2,
    },
    Decomposition {
        point: 0x1EE9,
        offset: 1280,
        len: 2,
    },
    Decomposition {
        point: 0x1EEA,
        offset: 1282,
        len: 2,
    },
    Decomposition {
        point: 0x1EEB,
        offset: 1284,
        len: 2,
    },
    Decomposition {
        point: 0x1EEC,
        offset: 1286,
        len: 2,
    },
    Decomposition {
        point: 0x1EED,
        offset: 1288,
        len: 2,
    },
    Decomposition {
        point: 0x1EEE,
        offset: 1290,
        len: 2,
    },
    Decomposition {
        point: 0x1EEF,
        offset: 1292,
        len: 2,
    },
    Decomposition {
        point: 0x1EF0,
        offset: 1294,
        len: 2,
    },
    Decomposition {
        point: 0x1EF1,
        offset: 1296,
        len: 2,
    },
    Decomposition {
        point: 0x1EF2,
        offset: 1298,
        len: 2,
    },
    Decomposition {
        point: 0x1EF3,
        offset: 1300,
        len: 2,
    },
    Decomposition {
        point: 0x1EF4,
        offset: 1302,
        len: 2,
    },
    Decomposition {
        point: 0x1EF5,
        offset: 1304,
        len: 2,
    },
    Decomposition {
        point: 0x1EF6,
        offset: 1306,
        len: 2,
    },
    Decomposition {
        point: 0x1EF7,
        offset: 1308,
        len: 2,
    },
    Decomposition {
        point: 0x1EF8,
        offset: 1310,
        len: 2,
    },
    Decomposition {
        point: 0x1EF9,
        offset: 1312,
        len: 2,
    },
    Decomposition {
        point: 0x1F00,
        offset: 1314,
        len: 2,
    },
    Decomposition {
        point: 0x1F01,
        offset: 1316,
        len: 2,
    },
    Decomposition {
        point: 0x1F02,
        offset: 1318,
        len: 2,
    },
    Decomposition {
        point: 0x1F03,
        offset: 1320,
        len: 2,
    },
    Decomposition {
        point: 0x1F04,
        offset: 1322,
        len: 2,
    },
    Decomposition {
        point: 0x1F05,
        offset: 1324,
        len: 2,
    },
    Decomposition {
        point: 0x1F06,
        offset: 1326,
        len: 2,
    },
    Decomposition {
        point: 0x1F07,
        offset: 1328,
        len: 2,
    },
    Decomposition {
        point: 0x1F08,
        offset: 1330,
        len: 2,
    },
    Decomposition {
        point: 0x1F09,
        offset: 1332,
        len: 2,
    },
    Decomposition {
        point: 0x1F0A,
        offset: 1334,
        len: 2,
    },
    Decomposition {
        point: 0x1F0B,
        offset: 1336,
        len: 2,
    },
    Decomposition {
        point: 0x1F0C,
        offset: 1338,
        len: 2,
    },
    Decomposition {
        point: 0x1F0D,
        offset: 1340,
        len: 2,
    },
    Decomposition {
        point: 0x1F0E,
        offset: 1342,
        len: 2,
    },
    Decomposition {
        point: 0x1F0F,
        offset: 1344,
        len: 2,
    },
    Decomposition {
        point: 0x1F10,
        offset: 1346,
        len: 2,
    },
    Decomposition {
        point: 0x1F11,
        offset: 1348,
        len: 2,
    },
    Decomposition {
        point: 0x1F12,
        offset: 1350,
        len: 2,
    },
    Decomposition {
        point: 0x1F13,
        offset: 1352,
        len: 2,
    },
    Decomposition {
        point: 0x1F14,
        offset: 1354,
        len: 2,
    },
    Decomposition {
        point: 0x1F15,
        offset: 1356,
        len: 2,
    },
    Decomposition {
        point: 0x1F18,
        offset: 1358,
        len: 2,
    },
    Decomposition {
        point: 0x1F19,
        offset: 1360,
        len: 2,
    },
    Decomposition {
        point: 0x1F1A,
        offset: 1362,
        len: 2,
    },
    Decomposition {
        point: 0x1F1B,
        offset: 1364,
        len: 2,
    },
    Decomposition {
        point: 0x1F1C,
        offset: 1366,
        len: 2,
    },
    Decomposition {
        point: 0x1F1D,
        offset: 1368,
        len: 2,
    },
    Decomposition {
        point: 0x1F20,
        offset: 1370,
        len: 2,
    },
    Decomposition {
        point: 0x1F21,
        offset: 1372,
        len: 2,
    },
    Decomposition {
        point: 0x1F22,
        offset: 1374,
        len: 2,
    },
    Decomposition {
        point: 0x1F23,
        offset: 1376,
        len: 2,
    },
    Decomposition {
        point: 0x1F24,
        offset: 1378,
        len: 2,
    },
    Decomposition {
        point: 0x1F25,
        offset: 1380,
        len: 2,
    },
    Decomposition {
        point: 0x1F26,
        offset: 1382,
        len: 2,
    },
    Decomposition {
        point: 0x1F27,
        offset: 1384,
        len: 2,
    },
    Decomposition {
        point: 0x1F28,
        offset: 1386,
        len: 2,
    },
    Decomposition {
        point: 0x1F29,
        offset: 1388,
        len: 2,
    },
    Decomposition {
        point: 0x1F2A,
        offset: 1390,
        len: 2,
    },
    Decomposition {
        point: 0x1F2B,
        offset: 1392,
        len: 2,
    },
    Decomposition {
        point: 0x1F2C,
        offset: 1394,
        len: 2,
    },
    Decomposition {
        point: 0x1F2D,
        offset: 1396,
        len: 2,
    },
    Decomposition {
        point: 0x1F2E,
        offset: 1398,
        len: 2,
    },
    Decomposition {
        point: 0x1F2F,
        offset: 1400,
        len: 2,
    },
    Decomposition {
        point: 0x1F30,
        offset: 1402,
        len: 2,
    },
    Decomposition {
        point: 0x1F31,
        offset: 1404,
        len: 2,
    },
    Decomposition {
        point: 0x1F32,
        offset: 1406,
        len: 2,
    },
    Decomposition {
        point: 0x1F33,
        offset: 1408,
        len: 2,
    },
    Decomposition {
        point: 0x1F34,
        offset: 1410,
        len: 2,
    },
    Decomposition {
        point: 0x1F35,
        offset: 1412,
        len: 2,
    },
    Decomposition {
        point: 0x1F36,
        offset: 1414,
        len: 2,
    },
    Decomposition {
        point: 0x1F37,
        offset: 1416,
        len: 2,
    },
    Decomposition {
        point: 0x1F38,
        offset: 1418,
        len: 2,
    },
    Decomposition {
        point: 0x1F39,
        offset: 1420,
        len: 2,
    },
    Decomposition {
        point: 0x1F3A,
        offset: 1422,
        len: 2,
    },
    Decomposition {
        point: 0x1F3B,
        offset: 1424,
        len: 2,
    },
    Decomposition {
        point: 0x1F3C,
        offset: 1426,
        len: 2,
    },
    Decomposition {
        point: 0x1F3D,
        offset: 1428,
        len: 2,
    },
    Decomposition {
        point: 0x1F3E,
        offset: 1430,
        len: 2,
    },
    Decomposition {
        point: 0x1F3F,
        offset: 1432,
        len: 2,
    },
    Decomposition {
        point: 0x1F40,
        offset: 1434,
        len: 2,
    },
    Decomposition {
        point: 0x1F41,
        offset: 1436,
        len: 2,
    },
    Decomposition {
        point: 0x1F42,
        offset: 1438,
        len: 2,
    },
    Decomposition {
        point: 0x1F43,
        offset: 1440,
        len: 2,
    },
    Decomposition {
        point: 0x1F44,
        offset: 1442,
        len: 2,
    },
    Decomposition {
        point: 0x1F45,
        offset: 1444,
        len: 2,
    },
    Decomposition {
        point: 0x1F48,
        offset: 1446,
        len: 2,
    },
    Decomposition {
        point: 0x1F49,
        offset: 1448,
        len: 2,
    },
    Decomposition {
        point: 0x1F4A,
        offset: 1450,
        len: 2,
    },
    Decomposition {
        point: 0x1F4B,
        offset: 1452,
        len: 2,
    },
    Decomposition {
        point: 0x1F4C,
        offset: 1454,
        len: 2,
    },
    Decomposition {
        point: 0x1F4D,
        offset: 1456,
        len: 2,
    },
    Decomposition {
        point: 0x1F50,
        offset: 1458,
        len: 2,
    },
    Decomposition {
        point: 0x1F51,
        offset: 1460,
        len: 2,
    },
    Decomposition {
        point: 0x1F52,
        offset: 1462,
        len: 2,
    },
    Decomposition {
        point: 0x1F53,
        offset: 1464,
        len: 2,
    },
    Decomposition {
        point: 0x1F54,
        offset: 1466,
        len: 2,
    },
    Decomposition {
        point: 0x1F55,
        offset: 1468,
        len: 2,
    },
    Decomposition {
        point: 0x1F56,
        offset: 1470,
        len: 2,
    },
    Decomposition {
        point: 0x1F57,
        offset: 1472,
        len: 2,
    },
    Decomposition {
        point: 0x1F59,
        offset: 1474,
        len: 2,
    },
    Decomposition {
        point: 0x1F5B,
        offset: 1476,
        len: 2,
    },
    Decomposition {
        point: 0x1F5D,
        offset: 1478,
        len: 2,
    },
    Decomposition {
        point: 0x1F5F,
        offset: 1480,
        len: 2,
    },
    Decomposition {
        point: 0x1F60,
        offset: 1482,
        len: 2,
    },
    Decomposition {
        point: 0x1F61,
        offset: 1484,
        len: 2,
    },
    Decomposition {
        point: 0x1F62,
        offset: 1486,
        len: 2,
    },
    Decomposition {
        point: 0x1F63,
        offset: 1488,
        len: 2,
    },
    Decomposition {
        point: 0x1F64,
        offset: 1490,
        len: 2,
    },
    Decomposition {
        point: 0x1F65,
        offset: 1492,
        len: 2,
    },
    Decomposition {
        point: 0x1F66,
        offset: 1494,
        len: 2,
    },
    Decomposition {
        point: 0x1F67,
        offset: 1496,
        len: 2,
    },
    Decomposition {
        point: 0x1F68,
        offset: 1498,
        len: 2,
    },
    Decomposition {
        point: 0x1F69,
        offset: 1500,
        len: 2,
    },
    Decomposition {
        point: 0x1F6A,
        offset: 1502,
        len: 2,
    },
    Decomposition {
        point: 0x1F6B,
        offset: 1504,
        len: 2,
    },
    Decomposition {
        point: 0x1F6C,
        offset: 1506,
        len: 2,
    },
    Decomposition {
        point: 0x1F6D,
        offset: 1508,
        len: 2,
    },
    Decomposition {
        point: 0x1F6E,
        offset: 1510,
        len: 2,
    },
    Decomposition {
        point: 0x1F6F,
        offset: 1512,
        len: 2,
    },
    Decomposition {
        point: 0x1F70,
        offset: 1514,
        len: 2,
    },
    Decomposition {
        point: 0x1F71,
        offset: 1516,
        len: 1,
    },
    Decomposition {
        point: 0x1F72,
        offset: 1517,
        len: 2,
    },
    Decomposition {
        point: 0x1F73,
        offset: 1519,
        len: 1,
    },
    Decomposition {
        point: 0x1F74,
        offset: 1520,
        len: 2,
    },
    Decomposition {
        point: 0x1F75,
        offset: 1522,
        len: 1,
    },
    Decomposition {
        point: 0x1F76,
        offset: 1523,
        len: 2,
    },
    Decomposition {
        point: 0x1F77,
        offset: 1525,
        len: 1,
    },
    Decomposition {
        point: 0x1F78,
        offset: 1526,
        len: 2,
    },
    Decomposition {
        point: 0x1F79,
        offset: 1528,
        len: 1,
    },
    Decomposition {
        point: 0x1F7A,
        offset: 1529,
        len: 2,
    },
    Decomposition {
        point: 0x1F7B,
        offset: 1531,
        len: 1,
    },
    Decomposition {
        point: 0x1F7C,
        offset: 1532,
        len: 2,
    },
    Decomposition {
        point: 0x1F7D,
        offset: 1534,
        len: 1,
    },
    Decomposition {
        point: 0x1F80,
        offset: 1535,
        len: 2,
    },
    Decomposition {
        point: 0x1F81,
        offset: 1537,
        len: 2,
    },
    Decomposition {
        point: 0x1F82,
        offset: 1539,
        len: 2,
    },
    Decomposition {
        point: 0x1F83,
        offset: 1541,
        len: 2,
    },
    Decomposition {
        point: 0x1F84,
        offset: 1543,
        len: 2,
    },
    Decomposition {
        point: 0x1F85,
        offset: 1545,
        len: 2,
    },
    Decomposition {
        point: 0x1F86,
        offset: 1547,
        len: 2,
    },
    Decomposition {
        point: 0x1F87,
        offset: 1549,
        len: 2,
    },
    Decomposition {
        point: 0x1F88,
        offset: 1551,
        len: 2,
    },
    Decomposition {
        point: 0x1F89,
        offset: 1553,
        len: 2,
    },
    Decomposition {
        point: 0x1F8A,
        offset: 1555,
        len: 2,
    },
    Decomposition {
        point: 0x1F8B,
        offset: 1557,
        len: 2,
    },
    Decomposition {
        point: 0x1F8C,
        offset: 1559,
        len: 2,
    },
    Decomposition {
        point: 0x1F8D,
        offset: 1561,
        len: 2,
    },
    Decomposition {
        point: 0x1F8E,
        offset: 1563,
        len: 2,
    },
    Decomposition {
        point: 0x1F8F,
        offset: 1565,
        len: 2,
    },
    Decomposition {
        point: 0x1F90,
        offset: 1567,
        len: 2,
    },
    Decomposition {
        point: 0x1F91,
        offset: 1569,
        len: 2,
    },
    Decomposition {
        point: 0x1F92,
        offset: 1571,
        len: 2,
    },
    Decomposition {
        point: 0x1F93,
        offset: 1573,
        len: 2,
    },
    Decomposition {
        point: 0x1F94,
        offset: 1575,
        len: 2,
    },
    Decomposition {
        point: 0x1F95,
        offset: 1577,
        len: 2,
    },
    Decomposition {
        point: 0x1F96,
        offset: 1579,
        len: 2,
    },
    Decomposition {
        point: 0x1F97,
        offset: 1581,
        len: 2,
    },
    Decomposition {
        point: 0x1F98,
        offset: 1583,
        len: 2,
    },
    Decomposition {
        point: 0x1F99,
        offset: 1585,
        len: 2,
    },
    Decomposition {
        point: 0x1F9A,
        offset: 1587,
        len: 2,
    },
    Decomposition {
        point: 0x1F9B,
        offset: 1589,
        len: 2,
    },
    Decomposition {
        point: 0x1F9C,
        offset: 1591,
        len: 2,
    },
    Decomposition {
        point: 0x1F9D,
        offset: 1593,
        len: 2,
    },
    Decomposition {
        point: 0x1F9E,
        offset: 1595,
        len: 2,
    },
    Decomposition {
        point: 0x1F9F,
        offset: 1597,
        len: 2,
    },
    Decomposition {
        point: 0x1FA0,
        offset: 1599,
        len: 2,
    },
    Decomposition {
        point: 0x1FA1,
        offset: 1601,
        len: 2,
    },
    Decomposition {
        point: 0x1FA2,
        offset: 1603,
        len: 2,
    },
    Decomposition {
        point: 0x1FA3,
        offset: 1605,
        len: 2,
    },
    Decomposition {
        point: 0x1FA4,
        offset: 1607,
        len: 2,
    },
    Decomposition {
        point: 0x1FA5,
        offset: 1609,
        len: 2,
    },
    Decomposition {
        point: 0x1FA6,
        offset: 1611,
        len: 2,
    },
    Decomposition {
        point: 0x1FA7,
        offset: 1613,
        len: 2,
    },
    Decomposition {
        point: 0x1FA8,
        offset: 1615,
        len: 2,
    },
    Decomposition {
        point: 0x1FA9,
        offset: 1617,
        len: 2,
    },
    Decomposition {
        point: 0x1FAA,
        offset: 1619,
        len: 2,
    },
    Decomposition {
        point: 0x1FAB,
        offset: 1621,
        len: 2,
    },
    Decomposition {
        point: 0x1FAC,
        offset: 1623,
        len: 2,
    },
    Decomposition {
        point: 0x1FAD,
        offset: 1625,
        len: 2,
    },
    Decomposition {
        point: 0x1FAE,
        offset: 1627,
        len: 2,
    },
    Decomposition {
        point: 0x1FAF,
        offset: 1629,
        len: 2,
    },
    Decomposition {
        point: 0x1FB0,
        offset: 1631,
        len: 2,
    },
    Decomposition {
        point: 0x1FB1,
        offset: 1633,
        len: 2,
    },
    Decomposition {
        point: 0x1FB2,
        offset: 1635,
        len: 2,
    },
    Decomposition {
        point: 0x1FB3,
        offset: 1637,
        len: 2,
    },
    Decomposition {
        point: 0x1FB4,
        offset: 1639,
        len: 2,
    },
    Decomposition {
        point: 0x1FB6,
        offset: 1641,
        len: 2,
    },
    Decomposition {
        point: 0x1FB7,
        offset: 1643,
        len: 2,
    },
    Decomposition {
        point: 0x1FB8,
        offset: 1645,
        len: 2,
    },
    Decomposition {
        point: 0x1FB9,
        offset: 1647,
        len: 2,
    },
    Decomposition {
        point: 0x1FBA,
        offset: 1649,
        len: 2,
    },
    Decomposition {
        point: 0x1FBB,
        offset: 1651,
        len: 1,
    },
    Decomposition {
        point: 0x1FBC,
        offset: 1652,
        len: 2,
    },
    Decomposition {
        point: 0x1FBE,
        offset: 1654,
        len: 1,
    },
    Decomposition {
        point: 0x1FC1,
        offset: 1655,
        len: 2,
    },
    Decomposition {
        point: 0x1FC2,
        offset: 1657,
        len: 2,
    },
    Decomposition {
        point: 0x1FC3,
        offset: 1659,
        len: 2,
    },
    Decomposition {
        point: 0x1FC4,
        offset: 1661,
        len: 2,
    },
    Decomposition {
        point: 0x1FC6,
        offset: 1663,
        len: 2,
    },
    Decomposition {
        point: 0x1FC7,
        offset: 1665,
        len: 2,
    },
    Decomposition {
        point: 0x1FC8,
        offset: 1667,
        len: 2,
    },
    Decomposition {
        point: 0x1FC9,
        offset: 1669,
        len: 1,
    },
    Decomposition {
        point: 0x1FCA,
        offset: 1670,
        len: 2,
    },
    Decomposition {
        point: 0x1FCB,
        offset: 1672,
        len: 1,
    },
    Decomposition {
        point: 0x1FCC,
        offset: 1673,
        len: 2,
    },
    Decomposition {
        point: 0x1FCD,
        offset: 1675,
        len: 2,
    },
    Decomposition {
        point: 0x1FCE,
        offset: 1677,
        len: 2,
    },
    Decomposition {
        point: 0x1FCF,
        offset: 1679,
        len: 2,
    },
    Decomposition {
        point: 0x1FD0,
        offset: 1681,
        len: 2,
    },
    Decomposition {
        point: 0x1FD1,
        offset: 1683,
        len: 2,
    },
    Decomposition {
        point: 0x1FD2,
        offset: 1685,
        len: 2,
    },
    Decomposition {
        point: 0x1FD3,
        offset: 1687,
        len: 1,
    },
    Decomposition {
        point: 0x1FD6,
        offset: 1688,
        len: 2,
    },
    Decomposition {
        point: 0x1FD7,
        offset: 1690,
        len: 2,
    },
    Decomposition {
        point: 0x1FD8,
        offset: 1692,
        len: 2,
    },
    Decomposition {
        point: 0x1FD9,
        offset: 1694,
        len: 2,
    },
    Decomposition {
        point: 0x1FDA,
        offset: 1696,
        len: 2,
    },
    Decomposition {
        point: 0x1FDB,
        offset: 1698,
        len: 1,
    },
    Decomposition {
        point: 0x1FDD,
        offset: 1699,
        len: 2,
    },
    Decomposition {
        point: 0x1FDE,
        offset: 1701,
        len: 2,
    },
    Decomposition {
        point: 0x1FDF,
        offset: 1703,
        len: 2,
    },
    Decomposition {
        point: 0x1FE0,
        offset: 1705,
        len: 2,
    },
    Decomposition {
        point: 0x1FE1,
        offset: 1707,
        len: 2,
    },
    Decomposition {
        point: 0x1FE2,
        offset: 1709,
        len: 2,
    },
    Decomposition {
        point: 0x1FE3,
        offset: 1711,
        len: 1,
    },
    Decomposition {
        point: 0x1FE4,
        offset: 1712,
        len: 2,
    },
    Decomposition {
        point: 0x1FE5,
        offset: 1714,
        len: 2,
    },
    Decomposition {
        point: 0x1FE6,
        offset: 1716,
        len: 2,
    },
    Decomposition {
        point: 0x1FE7,
        offset: 1718,
        len: 2,
    },
    Decomposition {
        point: 0x1FE8,
        offset: 1720,
        len: 2,
    },
    Decomposition {
        point: 0x1FE9,
        offset: 1722,
        len: 2,
    },
    Decomposition {
        point: 0x1FEA,
        offset: 1724,
        len: 2,
    },
    Decomposition {
        point: 0x1FEB,
        offset: 1726,
        len: 1,
    },
    Decomposition {
        point: 0x1FEC,
        offset: 1727,
        len: 2,
    },
    Decomposition {
        point: 0x1FED,
        offset: 1729,
        len: 2,
    },
    Decomposition {
        point: 0x1FEE,
        offset: 1731,
        len: 1,
    },
    Decomposition {
        point: 0x1FEF,
        offset: 1732,
        len: 1,
    },
    Decomposition {
        point: 0x1FF2,
        offset: 1733,
        len: 2,
    },
    Decomposition {
        point: 0x1FF3,
        offset: 1735,
        len: 2,
    },
    Decomposition {
        point: 0x1FF4,
        offset: 1737,
        len: 2,
    },
    Decomposition {
        point: 0x1FF6,
        offset: 1739,
        len: 2,
    },
    Decomposition {
        point: 0x1FF7,
        offset: 1741,
        len: 2,
    },
    Decomposition {
        point: 0x1FF8,
        offset: 1743,
        len: 2,
    },
    Decomposition {
        point: 0x1FF9,
        offset: 1745,
        len: 1,
    },
    Decomposition {
        point: 0x1FFA,
        offset: 1746,
        len: 2,
    },
    Decomposition {
        point: 0x1FFB,
        offset: 1748,
        len: 1,
    },
    Decomposition {
        point: 0x1FFC,
        offset: 1749,
        len: 2,
    },
    Decomposition {
        point: 0x1FFD,
        offset: 1751,
        len: 1,
    },
    Decomposition {
        point: 0x2000,
        offset: 1752,
        len: 1,
    },
    Decomposition {
        point: 0x2001,
        offset: 1753,
        len: 1,
    },
    Decomposition {
        point: 0x2126,
        offset: 1754,
        len: 1,
    },
    Decomposition {
        point: 0x212A,
        offset: 1755,
        len: 1,
    },
    Decomposition {
        point: 0x212B,
        offset: 1756,
        len: 1,
    },
    Decomposition {
        point: 0x219A,
        offset: 1757,
        len: 2,
    },
    Decomposition {
        point: 0x219B,
        offset: 1759,
        len: 2,
    },
    Decomposition {
        point: 0x21AE,
        offset: 1761,
        len: 2,
    },
    Decomposition {
        point: 0x21CD,
        offset: 1763,
        len: 2,
    },
    Decomposition {
        point: 0x21CE,
        offset: 1765,
        len: 2,
    },
    Decomposition {
        point: 0x21CF,
        offset: 1767,
        len: 2,
    },
    Decomposition {
        point: 0x2204,
        offset: 1769,
        len: 2,
    },
    Decomposition {
        point: 0x2209,
        offset: 1771,
        len: 2,
    },
    Decomposition {
        point: 0x220C,
        offset: 1773,
        len: 2,
    },
    Decomposition {
        point: 0x2224,
        offset: 1775,
        len: 2,
    },
    Decomposition {
        point: 0x2226,
        offset: 1777,
        len: 2,
    },
    Decomposition {
        point: 0x2241,
        offset: 1779,
        len: 2,
    },
    Decomposition {
        point: 0x2244,
        offset: 1781,
        len: 2,
    },
    Decomposition {
        point: 0x2247,
        offset: 1783,
        len: 2,
    },
    Decomposition {
        point: 0x2249,
        offset: 1785,
        len: 2,
    },
    Decomposition {
        point: 0x2260,
        offset: 1787,
        len: 2,
    },
    Decomposition {
        point: 0x2262,
        offset: 1789,
        len: 2,
    },
    Decomposition {
        point: 0x226D,
        offset: 1791,
        len: 2,
    },
    Decomposition {
        point: 0x226E,
        offset: 1793,
        len: 2,
    },
    Decomposition {
        point: 0x226F,
        offset: 1795,
        len: 2,
    },
    Decomposition {
        point: 0x2270,
        offset: 1797,
        len: 2,
    },
    Decomposition {
        point: 0x2271,
        offset: 1799,
        len: 2,
    },
    Decomposition {
        point: 0x2274,
        offset: 1801,
        len: 2,
    },
    Decomposition {
        point: 0x2275,
        offset: 1803,
        len: 2,
    },
    Decomposition {
        point: 0x2278,
        offset: 1805,
        len: 2,
    },
    Decomposition {
        point: 0x2279,
        offset: 1807,
        len: 2,
    },
    Decomposition {
        point: 0x2280,
        offset: 1809,
        len: 2,
    },
    Decomposition {
        point: 0x2281,
        offset: 1811,
        len: 2,
    },
    Decomposition {
        point: 0x2284,
        offset: 1813,
        len: 2,
    },
    Decomposition {
        point: 0x2285,
        offset: 1815,
        len: 2,
    },
    Decomposition {
        point: 0x2288,
        offset: 1817,
        len: 2,
    },
    Decomposition {
        point: 0x2289,
        offset: 1819,
        len: 2,
    },
    Decomposition {
        point: 0x22AC,
        offset: 1821,
        len: 2,
    },
    Decomposition {
        point: 0x22AD,
        offset: 1823,
        len: 2,
    },
    Decomposition {
        point: 0x22AE,
        offset: 1825,
        len: 2,
    },
    Decomposition {
        point: 0x22AF,
        offset: 1827,
        len: 2,
    },
    Decomposition {
        point: 0x22E0,
        offset: 1829,
        len: 2,
    },
    Decomposition {
        point: 0x22E1,
        offset: 1831,
        len: 2,
    },
    Decomposition {
        point: 0x22E2,
        offset: 1833,
        len: 2,
    },
    Decomposition {
        point: 0x22E3,
        offset: 1835,
        len: 2,
    },
    Decomposition {
        point: 0x22EA,
        offset: 1837,
        len: 2,
    },
    Decomposition {
        point: 0x22EB,
        offset: 1839,
        len: 2,
    },
    Decomposition {
        point: 0x22EC,
        offset: 1841,
        len: 2,
    },
    Decomposition {
        point: 0x22ED,
        offset: 1843,
        len: 2,
    },
    Decomposition {
        point: 0x2329,
        offset: 1845,
        len: 1,
    },
    Decomposition {
        point: 0x232A,
        offset: 1846,
        len: 1,
    },
    Decomposition {
        point: 0x2ADC,
        offset: 1847,
        len: 2,
    },
    Decomposition {
        point: 0x304C,
        offset: 1849,
        len: 2,
    },
    Decomposition {
        point: 0x304E,
        offset: 1851,
        len: 2,
    },
    Decomposition {
        point: 0x3050,
        offset: 1853,
        len: 2,
    },
    Decomposition {
        point: 0x3052,
        offset: 1855,
        len: 2,
    },
    Decomposition {
        point: 0x3054,
        offset: 1857,
        len: 2,
    },
    Decomposition {
        point: 0x3056,
        offset: 1859,
        len: 2,
    },
    Decomposition {
        point: 0x3058,
        offset: 1861,
        len: 2,
    },
    Decomposition {
        point: 0x305A,
        offset: 1863,
        len: 2,
    },
    Decomposition {
        point: 0x305C,
        offset: 1865,
        len: 2,
    },
    Decomposition {
        point: 0x305E,
        offset: 1867,
        len: 2,
    },
    Decomposition {
        point: 0x3060,
        offset: 1869,
        len: 2,
    },
    Decomposition {
        point: 0x3062,
        offset: 1871,
        len: 2,
    },
    Decomposition {
        point: 0x3065,
        offset: 1873,
        len: 2,
    },
    Decomposition {
        point: 0x3067,
        offset: 1875,
        len: 2,
    },
    Decomposition {
        point: 0x3069,
        offset: 1877,
        len: 2,
    },
    Decomposition {
        point: 0x3070,
        offset: 1879,
        len: 2,
    },
    Decomposition {
        point: 0x3071,
        offset: 1881,
        len: 2,
    },
    Decomposition {
        point: 0x3073,
        offset: 1883,
        len: 2,
    },
    Decomposition {
        point: 0x3074,
        offset: 1885,
        len: 2,
    },
    Decomposition {
        point: 0x3076,
        offset: 1887,
        len: 2,
    },
    Decomposition {
        point: 0x3077,
        offset: 1889,
        len: 2,
    },
    Decomposition {
        point: 0x3079,
        offset: 1891,
        len: 2,
    },
    Decomposition {
        point: 0x307A,
        offset: 1893,
        len: 2,
    },
    Decomposition {
        point: 0x307C,
        offset: 1895,
        len: 2,
    },
    Decomposition {
        point: 0x307D,
        offset: 1897,
        len: 2,
    },
    Decomposition {
        point: 0x3094,
        offset: 1899,
        len: 2,
    },
    Decomposition {
        point: 0x309E,
        offset: 1901,
        len: 2,
    },
    Decomposition {
        point: 0x30AC,
        offset: 1903,
        len: 2,
    },
    Decomposition {
        point: 0x30AE,
        offset: 1905,
        len: 2,
    },
    Decomposition {
        point: 0x30B0,
        offset: 1907,
        len: 2,
    },
    Decomposition {
        point: 0x30B2,
        offset: 1909,
        len: 2,
    },
    Decomposition {
        point: 0x30B4,
        offset: 1911,
        len: 2,
    },
    Decomposition {
        point: 0x30B6,
        offset: 1913,
        len: 2,
    },
    Decomposition {
        point: 0x30B8,
        offset: 1915,
        len: 2,
    },
    Decomposition {
        point: 0x30BA,
        offset: 1917,
        len: 2,
    },
    Decomposition {
        point: 0x30BC,
        offset: 1919,
        len: 2,
    },
    Decomposition {
        point: 0x30BE,
        offset: 1921,
        len: 2,
    },
    Decomposition {
        point: 0x30C0,
        offset: 1923,
        len: 2,
    },
    Decomposition {
        point: 0x30C2,
        offset: 1925,
        len: 2,
    },
    Decomposition {
        point: 0x30C5,
        offset: 1927,
        len: 2,
    },
    Decomposition {
        point: 0x30C7,
        offset: 1929,
        len: 2,
    },
    Decomposition {
        point: 0x30C9,
        offset: 1931,
        len: 2,
    },
    Decomposition {
        point: 0x30D0,
        offset: 1933,
        len: 2,
    },
    Decomposition {
        point: 0x30D1,
        offset: 1935,
        len: 2,
    },
    Decomposition {
        point: 0x30D3,
        offset: 1937,
        len: 2,
    },
    Decomposition {
        point: 0x30D4,
        offset: 1939,
        len: 2,
    },
    Decomposition {
        point: 0x30D6,
        offset: 1941,
        len: 2,
    },
    Decomposition {
        point: 0x30D7,
        offset: 1943,
        len: 2,
    },
    Decomposition {
        point: 0x30D9,
        offset: 1945,
        len: 2,
    },
    Decomposition {
        point: 0x30DA,
        offset: 1947,
        len: 2,
    },
    Decomposition {
        point: 0x30DC,
        offset: 1949,
        len: 2,
    },
    Decomposition {
        point: 0x30DD,
        offset: 1951,
        len: 2,
    },
    Decomposition {
        point: 0x30F4,
        offset: 1953,
        len: 2,
    },
    Decomposition {
        point: 0x30F7,
        offset: 1955,
        len: 2,
    },
    Decomposition {
        point: 0x30F8,
        offset: 1957,
        len: 2,
    },
    Decomposition {
        point: 0x30F9,
        offset: 1959,
        len: 2,
    },
    Decomposition {
        point: 0x30FA,
        offset: 1961,
        len: 2,
    },
    Decomposition {
        point: 0x30FE,
        offset: 1963,
        len: 2,
    },
    Decomposition {
        point: 0xF900,
        offset: 1965,
        len: 1,
    },
    Decomposition {
        point: 0xF901,
        offset: 1966,
        len: 1,
    },
    Decomposition {
        point: 0xF902,
        offset: 1967,
        len: 1,
    },
    Decomposition {
        point: 0xF903,
        offset: 1968,
        len: 1,
    },
    Decomposition {
        point: 0xF904,
        offset: 1969,
        len: 1,
    },
    Decomposition {
        point: 0xF905,
        offset: 1970,
        len: 1,
    },
    Decomposition {
        point: 0xF906,
        offset: 1971,
        len: 1,
    },
    Decomposition {
        point: 0xF907,
        offset: 1972,
        len: 1,
    },
    Decomposition {
        point: 0xF908,
        offset: 1973,
        len: 1,
    },
    Decomposition {
        point: 0xF909,
        offset: 1974,
        len: 1,
    },
    Decomposition {
        point: 0xF90A,
        offset: 1975,
        len: 1,
    },
    Decomposition {
        point: 0xF90B,
        offset: 1976,
        len: 1,
    },
    Decomposition {
        point: 0xF90C,
        offset: 1977,
        len: 1,
    },
    Decomposition {
        point: 0xF90D,
        offset: 1978,
        len: 1,
    },
    Decomposition {
        point: 0xF90E,
        offset: 1979,
        len: 1,
    },
    Decomposition {
        point: 0xF90F,
        offset: 1980,
        len: 1,
    },
    Decomposition {
        point: 0xF910,
        offset: 1981,
        len: 1,
    },
    Decomposition {
        point: 0xF911,
        offset: 1982,
        len: 1,
    },
    Decomposition {
        point: 0xF912,
        offset: 1983,
        len: 1,
    },
    Decomposition {
        point: 0xF913,
        offset: 1984,
        len: 1,
    },
    Decomposition {
        point: 0xF914,
        offset: 1985,
        len: 1,
    },
    Decomposition {
        point: 0xF915,
        offset: 1986,
        len: 1,
    },
    Decomposition {
        point: 0xF916,
        offset: 1987,
        len: 1,
    },
    Decomposition {
        point: 0xF917,
        offset: 1988,
        len: 1,
    },
    Decomposition {
        point: 0xF918,
        offset: 1989,
        len: 1,
    },
    Decomposition {
        point: 0xF919,
        offset: 1990,
        len: 1,
    },
    Decomposition {
        point: 0xF91A,
        offset: 1991,
        len: 1,
    },
    Decomposition {
        point: 0xF91B,
        offset: 1992,
        len: 1,
    },
    Decomposition {
        point: 0xF91C,
        offset: 1993,
        len: 1,
    },
    Decomposition {
        point: 0xF91D,
        offset: 1994,
        len: 1,
    },
    Decomposition {
        point: 0xF91E,
        offset: 1995,
        len: 1,
    },
    Decomposition {
        point: 0xF91F,
        offset: 1996,
        len: 1,
    },
    Decomposition {
        point: 0xF920,
        offset: 1997,
        len: 1,
    },
    Decomposition {
        point: 0xF921,
        offset: 1998,
        len: 1,
    },
    Decomposition {
        point: 0xF922,
        offset: 1999,
        len: 1,
    },
    Decomposition {
        point: 0xF923,
        offset: 2000,
        len: 1,
    },
    Decomposition {
        point: 0xF924,
        offset: 2001,
        len: 1,
    },
    Decomposition {
        point: 0xF925,
        offset: 2002,
        len: 1,
    },
    Decomposition {
        point: 0xF926,
        offset: 2003,
        len: 1,
    },
    Decomposition {
        point: 0xF927,
        offset: 2004,
        len: 1,
    },
    Decomposition {
        point: 0xF928,
        offset: 2005,
        len: 1,
    },
    Decomposition {
        point: 0xF929,
        offset: 2006,
        len: 1,
    },
    Decomposition {
        point: 0xF92A,
        offset: 2007,
        len: 1,
    },
    Decomposition {
        point: 0xF92B,
        offset: 2008,
        len: 1,
    },
    Decomposition {
        point: 0xF92C,
        offset: 2009,
        len: 1,
    },
    Decomposition {
        point: 0xF92D,
        offset: 2010,
        len: 1,
    },
    Decomposition {
        point: 0xF92E,
        offset: 2011,
        len: 1,
    },
    Decomposition {
        point: 0xF92F,
        offset: 2012,
        len: 1,
    },
    Decomposition {
        point: 0xF930,
        offset: 2013,
        len: 1,
    },
    Decomposition {
        point: 0xF931,
        offset: 2014,
        len: 1,
    },
    Decomposition {
        point: 0xF932,
        offset: 2015,
        len: 1,
    },
    Decomposition {
        point: 0xF933,
        offset: 2016,
        len: 1,
    },
    Decomposition {
        point: 0xF934,
        offset: 2017,
        len: 1,
    },
    Decomposition {
        point: 0xF935,
        offset: 2018,
        len: 1,
    },
    Decomposition {
        point: 0xF936,
        offset: 2019,
        len: 1,
    },
    Decomposition {
        point: 0xF937,
        offset: 2020,
        len: 1,
    },
    Decomposition {
        point: 0xF938,
        offset: 2021,
        len: 1,
    },
    Decomposition {
        point: 0xF939,
        offset: 2022,
        len: 1,
    },
    Decomposition {
        point: 0xF93A,
        offset: 2023,
        len: 1,
    },
    Decomposition {
        point: 0xF93B,
        offset: 2024,
        len: 1,
    },
    Decomposition {
        point: 0xF93C,
        offset: 2025,
        len: 1,
    },
    Decomposition {
        point: 0xF93D,
        offset: 2026,
        len: 1,
    },
    Decomposition {
        point: 0xF93E,
        offset: 2027,
        len: 1,
    },
    Decomposition {
        point: 0xF93F,
        offset: 2028,
        len: 1,
    },
    Decomposition {
        point: 0xF940,
        offset: 2029,
        len: 1,
    },
    Decomposition {
        point: 0xF941,
        offset: 2030,
        len: 1,
    },
    Decomposition {
        point: 0xF942,
        offset: 2031,
        len: 1,
    },
    Decomposition {
        point: 0xF943,
        offset: 2032,
        len: 1,
    },
    Decomposition {
        point: 0xF944,
        offset: 2033,
        len: 1,
    },
    Decomposition {
        point: 0xF945,
        offset: 2034,
        len: 1,
    },
    Decomposition {
        point: 0xF946,
        offset: 2035,
        len: 1,
    },
    Decomposition {
        point: 0xF947,
        offset: 2036,
        len: 1,
    },
    Decomposition {
        point: 0xF948,
        offset: 2037,
        len: 1,
    },
    Decomposition {
        point: 0xF949,
        offset: 2038,
        len: 1,
    },
    Decomposition {
        point: 0xF94A,
        offset: 2039,
        len: 1,
    },
    Decomposition {
        point: 0xF94B,
        offset: 2040,
        len: 1,
    },
    Decomposition {
        point: 0xF94C,
        offset: 2041,
        len: 1,
    },
    Decomposition {
        point: 0xF94D,
        offset: 2042,
        len: 1,
    },
    Decomposition {
        point: 0xF94E,
        offset: 2043,
        len: 1,
    },
    Decomposition {
        point: 0xF94F,
        offset: 2044,
        len: 1,
    },
    Decomposition {
        point: 0xF950,
        offset: 2045,
        len: 1,
    },
    Decomposition {
        point: 0xF951,
        offset: 2046,
        len: 1,
    },
    Decomposition {
        point: 0xF952,
        offset: 2047,
        len: 1,
    },
    Decomposition {
        point: 0xF953,
        offset: 2048,
        len: 1,
    },
    Decomposition {
        point: 0xF954,
        offset: 2049,
        len: 1,
    },
    Decomposition {
        point: 0xF955,
        offset: 2050,
        len: 1,
    },
    Decomposition {
        point: 0xF956,
        offset: 2051,
        len: 1,
    },
    Decomposition {
        point: 0xF957,
        offset: 2052,
        len: 1,
    },
    Decomposition {
        point: 0xF958,
        offset: 2053,
        len: 1,
    },
    Decomposition {
        point: 0xF959,
        offset: 2054,
        len: 1,
    },
    Decomposition {
        point: 0xF95A,
        offset: 2055,
        len: 1,
    },
    Decomposition {
        point: 0xF95B,
        offset: 2056,
        len: 1,
    },
    Decomposition {
        point: 0xF95C,
        offset: 2057,
        len: 1,
    },
    Decomposition {
        point: 0xF95D,
        offset: 2058,
        len: 1,
    },
    Decomposition {
        point: 0xF95E,
        offset: 2059,
        len: 1,
    },
    Decomposition {
        point: 0xF95F,
        offset: 2060,
        len: 1,
    },
    Decomposition {
        point: 0xF960,
        offset: 2061,
        len: 1,
    },
    Decomposition {
        point: 0xF961,
        offset: 2062,
        len: 1,
    },
    Decomposition {
        point: 0xF962,
        offset: 2063,
        len: 1,
    },
    Decomposition {
        point: 0xF963,
        offset: 2064,
        len: 1,
    },
    Decomposition {
        point: 0xF964,
        offset: 2065,
        len: 1,
    },
    Decomposition {
        point: 0xF965,
        offset: 2066,
        len: 1,
    },
    Decomposition {
        point: 0xF966,
        offset: 2067,
        len: 1,
    },
    Decomposition {
        point: 0xF967,
        offset: 2068,
        len: 1,
    },
    Decomposition {
        point: 0xF968,
        offset: 2069,
        len: 1,
    },
    Decomposition {
        point: 0xF969,
        offset: 2070,
        len: 1,
    },
    Decomposition {
        point: 0xF96A,
        offset: 2071,
        len: 1,
    },
    Decomposition {
        point: 0xF96B,
        offset: 2072,
        len: 1,
    },
    Decomposition {
        point: 0xF96C,
        offset: 2073,
        len: 1,
    },
    Decomposition {
        point: 0xF96D,
        offset: 2074,
        len: 1,
    },
    Decomposition {
        point: 0xF96E,
        offset: 2075,
        len: 1,
    },
    Decomposition {
        point: 0xF96F,
        offset: 2076,
        len: 1,
    },
    Decomposition {
        point: 0xF970,
        offset: 2077,
        len: 1,
    },
    Decomposition {
        point: 0xF971,
        offset: 2078,
        len: 1,
    },
    Decomposition {
        point: 0xF972,
        offset: 2079,
        len: 1,
    },
    Decomposition {
        point: 0xF973,
        offset: 2080,
        len: 1,
    },
    Decomposition {
        point: 0xF974,
        offset: 2081,
        len: 1,
    },
    Decomposition {
        point: 0xF975,
        offset: 2082,
        len: 1,
    },
    Decomposition {
        point: 0xF976,
        offset: 2083,
        len: 1,
    },
    Decomposition {
        point: 0xF977,
        offset: 2084,
        len: 1,
    },
    Decomposition {
        point: 0xF978,
        offset: 2085,
        len: 1,
    },
    Decomposition {
        point: 0xF979,
        offset: 2086,
        len: 1,
    },
    Decomposition {
        point: 0xF97A,
        offset: 2087,
        len: 1,
    },
    Decomposition {
        point: 0xF97B,
        offset: 2088,
        len: 1,
    },
    Decomposition {
        point: 0xF97C,
        offset: 2089,
        len: 1,
    },
    Decomposition {
        point: 0xF97D,
        offset: 2090,
        len: 1,
    },
    Decomposition {
        point: 0xF97E,
        offset: 2091,
        len: 1,
    },
    Decomposition {
        point: 0xF97F,
        offset: 2092,
        len: 1,
    },
    Decomposition {
        point: 0xF980,
        offset: 2093,
        len: 1,
    },
    Decomposition {
        point: 0xF981,
        offset: 2094,
        len: 1,
    },
    Decomposition {
        point: 0xF982,
        offset: 2095,
        len: 1,
    },
    Decomposition {
        point: 0xF983,
        offset: 2096,
        len: 1,
    },
    Decomposition {
        point: 0xF984,
        offset: 2097,
        len: 1,
    },
    Decomposition {
        point: 0xF985,
        offset: 2098,
        len: 1,
    },
    Decomposition {
        point: 0xF986,
        offset: 2099,
        len: 1,
    },
    Decomposition {
        point: 0xF987,
        offset: 2100,
        len: 1,
    },
    Decomposition {
        point: 0xF988,
        offset: 2101,
        len: 1,
    },
    Decomposition {
        point: 0xF989,
        offset: 2102,
        len: 1,
    },
    Decomposition {
        point: 0xF98A,
        offset: 2103,
        len: 1,
    },
    Decomposition {
        point: 0xF98B,
        offset: 2104,
        len: 1,
    },
    Decomposition {
        point: 0xF98C,
        offset: 2105,
        len: 1,
    },
    Decomposition {
        point: 0xF98D,
        offset: 2106,
        len: 1,
    },
    Decomposition {
        point: 0xF98E,
        offset: 2107,
        len: 1,
    },
    Decomposition {
        point: 0xF98F,
        offset: 2108,
        len: 1,
    },
    Decomposition {
        point: 0xF990,
        offset: 2109,
        len: 1,
    },
    Decomposition {
        point: 0xF991,
        offset: 2110,
        len: 1,
    },
    Decomposition {
        point: 0xF992,
        offset: 2111,
        len: 1,
    },
    Decomposition {
        point: 0xF993,
        offset: 2112,
        len: 1,
    },
    Decomposition {
        point: 0xF994,
        offset: 2113,
        len: 1,
    },
    Decomposition {
        point: 0xF995,
        offset: 2114,
        len: 1,
    },
    Decomposition {
        point: 0xF996,
        offset: 2115,
        len: 1,
    },
    Decomposition {
        point: 0xF997,
        offset: 2116,
        len: 1,
    },
    Decomposition {
        point: 0xF998,
        offset: 2117,
        len: 1,
    },
    Decomposition {
        point: 0xF999,
        offset: 2118,
        len: 1,
    },
    Decomposition {
        point: 0xF99A,
        offset: 2119,
        len: 1,
    },
    Decomposition {
        point: 0xF99B,
        offset: 2120,
        len: 1,
    },
    Decomposition {
        point: 0xF99C,
        offset: 2121,
        len: 1,
    },
    Decomposition {
        point: 0xF99D,
        offset: 2122,
        len: 1,
    },
    Decomposition {
        point: 0xF99E,
        offset: 2123,
        len: 1,
    },
    Decomposition {
        point: 0xF99F,
        offset: 2124,
        len: 1,
    },
    Decomposition {
        point: 0xF9A0,
        offset: 2125,
        len: 1,
    },
    Decomposition {
        point: 0xF9A1,
        offset: 2126,
        len: 1,
    },
    Decomposition {
        point: 0xF9A2,
        offset: 2127,
        len: 1,
    },
    Decomposition {
        point: 0xF9A3,
        offset: 2128,
        len: 1,
    },
    Decomposition {
        point: 0xF9A4,
        offset: 2129,
        len: 1,
    },
    Decomposition {
        point: 0xF9A5,
        offset: 2130,
        len: 1,
    },
    Decomposition {
        point: 0xF9A6,
        offset: 2131,
        len: 1,
    },
    Decomposition {
        point: 0xF9A7,
        offset: 2132,
        len: 1,
    },
    Decomposition {
        point: 0xF9A8,
        offset: 2133,
        len: 1,
    },
    Decomposition {
        point: 0xF9A9,
        offset: 2134,
        len: 1,
    },
    Decomposition {
        point: 0xF9AA,
        offset: 2135,
        len: 1,
    },
    Decomposition {
        point: 0xF9AB,
        offset: 2136,
        len: 1,
    },
    Decomposition {
        point: 0xF9AC,
        offset: 2137,
        len: 1,
    },
    Decomposition {
        point: 0xF9AD,
        offset: 2138,
        len: 1,
    },
    Decomposition {
        point: 0xF9AE,
        offset: 2139,
        len: 1,
    },
    Decomposition {
        point: 0xF9AF,
        offset: 2140,
        len: 1,
    },
    Decomposition {
        point: 0xF9B0,
        offset: 2141,
        len: 1,
    },
    Decomposition {
        point: 0xF9B1,
        offset: 2142,
        len: 1,
    },
    Decomposition {
        point: 0xF9B2,
        offset: 2143,
        len: 1,
    },
    Decomposition {
        point: 0xF9B3,
        offset: 2144,
        len: 1,
    },
    Decomposition {
        point: 0xF9B4,
        offset: 2145,
        len: 1,
    },
    Decomposition {
        point: 0xF9B5,
        offset: 2146,
        len: 1,
    },
    Decomposition {
        point: 0xF9B6,
        offset: 2147,
        len: 1,
    },
    Decomposition {
        point: 0xF9B7,
        offset: 2148,
        len: 1,
    },
    Decomposition {
        point: 0xF9B8,
        offset: 2149,
        len: 1,
    },
    Decomposition {
        point: 0xF9B9,
        offset: 2150,
        len: 1,
    },
    Decomposition {
        point: 0xF9BA,
        offset: 2151,
        len: 1,
    },
    Decomposition {
        point: 0xF9BB,
        offset: 2152,
        len: 1,
    },
    Decomposition {
        point: 0xF9BC,
        offset: 2153,
        len: 1,
    },
    Decomposition {
        point: 0xF9BD,
        offset: 2154,
        len: 1,
    },
    Decomposition {
        point: 0xF9BE,
        offset: 2155,
        len: 1,
    },
    Decomposition {
        point: 0xF9BF,
        offset: 2156,
        len: 1,
    },
    Decomposition {
        point: 0xF9C0,
        offset: 2157,
        len: 1,
    },
    Decomposition {
        point: 0xF9C1,
        offset: 2158,
        len: 1,
    },
    Decomposition {
        point: 0xF9C2,
        offset: 2159,
        len: 1,
    },
    Decomposition {
        point: 0xF9C3,
        offset: 2160,
        len: 1,
    },
    Decomposition {
        point: 0xF9C4,
        offset: 2161,
        len: 1,
    },
    Decomposition {
        point: 0xF9C5,
        offset: 2162,
        len: 1,
    },
    Decomposition {
        point: 0xF9C6,
        offset: 2163,
        len: 1,
    },
    Decomposition {
        point: 0xF9C7,
        offset: 2164,
        len: 1,
    },
    Decomposition {
        point: 0xF9C8,
        offset: 2165,
        len: 1,
    },
    Decomposition {
        point: 0xF9C9,
        offset: 2166,
        len: 1,
    },
    Decomposition {
        point: 0xF9CA,
        offset: 2167,
        len: 1,
    },
    Decomposition {
        point: 0xF9CB,
        offset: 2168,
        len: 1,
    },
    Decomposition {
        point: 0xF9CC,
        offset: 2169,
        len: 1,
    },
    Decomposition {
        point: 0xF9CD,
        offset: 2170,
        len: 1,
    },
    Decomposition {
        point: 0xF9CE,
        offset: 2171,
        len: 1,
    },
    Decomposition {
        point: 0xF9CF,
        offset: 2172,
        len: 1,
    },
    Decomposition {
        point: 0xF9D0,
        offset: 2173,
        len: 1,
    },
    Decomposition {
        point: 0xF9D1,
        offset: 2174,
        len: 1,
    },
    Decomposition {
        point: 0xF9D2,
        offset: 2175,
        len: 1,
    },
    Decomposition {
        point: 0xF9D3,
        offset: 2176,
        len: 1,
    },
    Decomposition {
        point: 0xF9D4,
        offset: 2177,
        len: 1,
    },
    Decomposition {
        point: 0xF9D5,
        offset: 2178,
        len: 1,
    },
    Decomposition {
        point: 0xF9D6,
        offset: 2179,
        len: 1,
    },
    Decomposition {
        point: 0xF9D7,
        offset: 2180,
        len: 1,
    },
    Decomposition {
        point: 0xF9D8,
        offset: 2181,
        len: 1,
    },
    Decomposition {
        point: 0xF9D9,
        offset: 2182,
        len: 1,
    },
    Decomposition {
        point: 0xF9DA,
        offset: 2183,
        len: 1,
    },
    Decomposition {
        point: 0xF9DB,
        offset: 2184,
        len: 1,
    },
    Decomposition {
        point: 0xF9DC,
        offset: 2185,
        len: 1,
    },
    Decomposition {
        point: 0xF9DD,
        offset: 2186,
        len: 1,
    },
    Decomposition {
        point: 0xF9DE,
        offset: 2187,
        len: 1,
    },
    Decomposition {
        point: 0xF9DF,
        offset: 2188,
        len: 1,
    },
    Decomposition {
        point: 0xF9E0,
        offset: 2189,
        len: 1,
    },
    Decomposition {
        point: 0xF9E1,
        offset: 2190,
        len: 1,
    },
    Decomposition {
        point: 0xF9E2,
        offset: 2191,
        len: 1,
    },
    Decomposition {
        point: 0xF9E3,
        offset: 2192,
        len: 1,
    },
    Decomposition {
        point: 0xF9E4,
        offset: 2193,
        len: 1,
    },
    Decomposition {
        point: 0xF9E5,
        offset: 2194,
        len: 1,
    },
    Decomposition {
        point: 0xF9E6,
        offset: 2195,
        len: 1,
    },
    Decomposition {
        point: 0xF9E7,
        offset: 2196,
        len: 1,
    },
    Decomposition {
        point: 0xF9E8,
        offset: 2197,
        len: 1,
    },
    Decomposition {
        point: 0xF9E9,
        offset: 2198,
        len: 1,
    },
    Decomposition {
        point: 0xF9EA,
        offset: 2199,
        len: 1,
    },
    Decomposition {
        point: 0xF9EB,
        offset: 2200,
        len: 1,
    },
    Decomposition {
        point: 0xF9EC,
        offset: 2201,
        len: 1,
    },
    Decomposition {
        point: 0xF9ED,
        offset: 2202,
        len: 1,
    },
    Decomposition {
        point: 0xF9EE,
        offset: 2203,
        len: 1,
    },
    Decomposition {
        point: 0xF9EF,
        offset: 2204,
        len: 1,
    },
    Decomposition {
        point: 0xF9F0,
        offset: 2205,
        len: 1,
    },
    Decomposition {
        point: 0xF9F1,
        offset: 2206,
        len: 1,
    },
    Decomposition {
        point: 0xF9F2,
        offset: 2207,
        len: 1,
    },
    Decomposition {
        point: 0xF9F3,
        offset: 2208,
        len: 1,
    },
    Decomposition {
        point: 0xF9F4,
        offset: 2209,
        len: 1,
    },
    Decomposition {
        point: 0xF9F5,
        offset: 2210,
        len: 1,
    },
    Decomposition {
        point: 0xF9F6,
        offset: 2211,
        len: 1,
    },
    Decomposition {
        point: 0xF9F7,
        offset: 2212,
        len: 1,
    },
    Decomposition {
        point: 0xF9F8,
        offset: 2213,
        len: 1,
    },
    Decomposition {
        point: 0xF9F9,
        offset: 2214,
        len: 1,
    },
    Decomposition {
        point: 0xF9FA,
        offset: 2215,
        len: 1,
    },
    Decomposition {
        point: 0xF9FB,
        offset: 2216,
        len: 1,
    },
    Decomposition {
        point: 0xF9FC,
        offset: 2217,
        len: 1,
    },
    Decomposition {
        point: 0xF9FD,
        offset: 2218,
        len: 1,
    },
    Decomposition {
        point: 0xF9FE,
        offset: 2219,
        len: 1,
    },
    Decomposition {
        point: 0xF9FF,
        offset: 2220,
        len: 1,
    },
    Decomposition {
        point: 0xFA00,
        offset: 2221,
        len: 1,
    },
    Decomposition {
        point: 0xFA01,
        offset: 2222,
        len: 1,
    },
    Decomposition {
        point: 0xFA02,
        offset: 2223,
        len: 1,
    },
    Decomposition {
        point: 0xFA03,
        offset: 2224,
        len: 1,
    },
    Decomposition {
        point: 0xFA04,
        offset: 2225,
        len: 1,
    },
    Decomposition {
        point: 0xFA05,
        offset: 2226,
        len: 1,
    },
    Decomposition {
        point: 0xFA06,
        offset: 2227,
        len: 1,
    },
    Decomposition {
        point: 0xFA07,
        offset: 2228,
        len: 1,
    },
    Decomposition {
        point: 0xFA08,
        offset: 2229,
        len: 1,
    },
    Decomposition {
        point: 0xFA09,
        offset: 2230,
        len: 1,
    },
    Decomposition {
        point: 0xFA0A,
        offset: 2231,
        len: 1,
    },
    Decomposition {
        point: 0xFA0B,
        offset: 2232,
        len: 1,
    },
    Decomposition {
        point: 0xFA0C,
        offset: 2233,
        len: 1,
    },
    Decomposition {
        point: 0xFA0D,
        offset: 2234,
        len: 1,
    },
    Decomposition {
        point: 0xFA10,
        offset: 2235,
        len: 1,
    },
    Decomposition {
        point: 0xFA12,
        offset: 2236,
        len: 1,
    },
    Decomposition {
        point: 0xFA15,
        offset: 2237,
        len: 1,
    },
    Decomposition {
        point: 0xFA16,
        offset: 2238,
        len: 1,
    },
    Decomposition {
        point: 0xFA17,
        offset: 2239,
        len: 1,
    },
    Decomposition {
        point: 0xFA18,
        offset: 2240,
        len: 1,
    },
    Decomposition {
        point: 0xFA19,
        offset: 2241,
        len: 1,
    },
    Decomposition {
        point: 0xFA1A,
        offset: 2242,
        len: 1,
    },
    Decomposition {
        point: 0xFA1B,
        offset: 2243,
        len: 1,
    },
    Decomposition {
        point: 0xFA1C,
        offset: 2244,
        len: 1,
    },
    Decomposition {
        point: 0xFA1D,
        offset: 2245,
        len: 1,
    },
    Decomposition {
        point: 0xFA1E,
        offset: 2246,
        len: 1,
    },
    Decomposition {
        point: 0xFA20,
        offset: 2247,
        len: 1,
    },
    Decomposition {
        point: 0xFA22,
        offset: 2248,
        len: 1,
    },
    Decomposition {
        point: 0xFA25,
        offset: 2249,
        len: 1,
    },
    Decomposition {
        point: 0xFA26,
        offset: 2250,
        len: 1,
    },
    Decomposition {
        point: 0xFA2A,
        offset: 2251,
        len: 1,
    },
    Decomposition {
        point: 0xFA2B,
        offset: 2252,
        len: 1,
    },
    Decomposition {
        point: 0xFA2C,
        offset: 2253,
        len: 1,
    },
    Decomposition {
        point: 0xFA2D,
        offset: 2254,
        len: 1,
    },
    Decomposition {
        point: 0xFA2E,
        offset: 2255,
        len: 1,
    },
    Decomposition {
        point: 0xFA2F,
        offset: 2256,
        len: 1,
    },
    Decomposition {
        point: 0xFA30,
        offset: 2257,
        len: 1,
    },
    Decomposition {
        point: 0xFA31,
        offset: 2258,
        len: 1,
    },
    Decomposition {
        point: 0xFA32,
        offset: 2259,
        len: 1,
    },
    Decomposition {
        point: 0xFA33,
        offset: 2260,
        len: 1,
    },
    Decomposition {
        point: 0xFA34,
        offset: 2261,
        len: 1,
    },
    Decomposition {
        point: 0xFA35,
        offset: 2262,
        len: 1,
    },
    Decomposition {
        point: 0xFA36,
        offset: 2263,
        len: 1,
    },
    Decomposition {
        point: 0xFA37,
        offset: 2264,
        len: 1,
    },
    Decomposition {
        point: 0xFA38,
        offset: 2265,
        len: 1,
    },
    Decomposition {
        point: 0xFA39,
        offset: 2266,
        len: 1,
    },
    Decomposition {
        point: 0xFA3A,
        offset: 2267,
        len: 1,
    },
    Decomposition {
        point: 0xFA3B,
        offset: 2268,
        len: 1,
    },
    Decomposition {
        point: 0xFA3C,
        offset: 2269,
        len: 1,
    },
    Decomposition {
        point: 0xFA3D,
        offset: 2270,
        len: 1,
    },
    Decomposition {
        point: 0xFA3E,
        offset: 2271,
        len: 1,
    },
    Decomposition {
        point: 0xFA3F,
        offset: 2272,
        len: 1,
    },
    Decomposition {
        point: 0xFA40,
        offset: 2273,
        len: 1,
    },
    Decomposition {
        point: 0xFA41,
        offset: 2274,
        len: 1,
    },
    Decomposition {
        point: 0xFA42,
        offset: 2275,
        len: 1,
    },
    Decomposition {
        point: 0xFA43,
        offset: 2276,
        len: 1,
    },
    Decomposition {
        point: 0xFA44,
        offset: 2277,
        len: 1,
    },
    Decomposition {
        point: 0xFA45,
        offset: 2278,
        len: 1,
    },
    Decomposition {
        point: 0xFA46,
        offset: 2279,
        len: 1,
    },
    Decomposition {
        point: 0xFA47,
        offset: 2280,
        len: 1,
    },
    Decomposition {
        point: 0xFA48,
        offset: 2281,
        len: 1,
    },
    Decomposition {
        point: 0xFA49,
        offset: 2282,
        len: 1,
    },
    Decomposition {
        point: 0xFA4A,
        offset: 2283,
        len: 1,
    },
    Decomposition {
        point: 0xFA4B,
        offset: 2284,
        len: 1,
    },
    Decomposition {
        point: 0xFA4C,
        offset: 2285,
        len: 1,
    },
    Decomposition {
        point: 0xFA4D,
        offset: 2286,
        len: 1,
    },
    Decomposition {
        point: 0xFA4E,
        offset: 2287,
        len: 1,
    },
    Decomposition {
        point: 0xFA4F,
        offset: 2288,
        len: 1,
    },
    Decomposition {
        point: 0xFA50,
        offset: 2289,
        len: 1,
    },
    Decomposition {
        point: 0xFA51,
        offset: 2290,
        len: 1,
    },
    Decomposition {
        point: 0xFA52,
        offset: 2291,
        len: 1,
    },
    Decomposition {
        point: 0xFA53,
        offset: 2292,
        len: 1,
    },
    Decomposition {
        point: 0xFA54,
        offset: 2293,
        len: 1,
    },
    Decomposition {
        point: 0xFA55,
        offset: 2294,
        len: 1,
    },
    Decomposition {
        point: 0xFA56,
        offset: 2295,
        len: 1,
    },
    Decomposition {
        point: 0xFA57,
        offset: 2296,
        len: 1,
    },
    Decomposition {
        point: 0xFA58,
        offset: 2297,
        len: 1,
    },
    Decomposition {
        point: 0xFA59,
        offset: 2298,
        len: 1,
    },
    Decomposition {
        point: 0xFA5A,
        offset: 2299,
        len: 1,
    },
    Decomposition {
        point: 0xFA5B,
        offset: 2300,
        len: 1,
    },
    Decomposition {
        point: 0xFA5C,
        offset: 2301,
        len: 1,
    },
    Decomposition {
        point: 0xFA5D,
        offset: 2302,
        len: 1,
    },
    Decomposition {
        point: 0xFA5E,
        offset: 2303,
        len: 1,
    },
    Decomposition {
        point: 0xFA5F,
        offset: 2304,
        len: 1,
    },
    Decomposition {
        point: 0xFA60,
        offset: 2305,
        len: 1,
    },
    Decomposition {
        point: 0xFA61,
        offset: 2306,
        len: 1,
    },
    Decomposition {
        point: 0xFA62,
        offset: 2307,
        len: 1,
    },
    Decomposition {
        point: 0xFA63,
        offset: 2308,
        len: 1,
    },
    Decomposition {
        point: 0xFA64,
        offset: 2309,
        len: 1,
    },
    Decomposition {
        point: 0xFA65,
        offset: 2310,
        len: 1,
    },
    Decomposition {
        point: 0xFA66,
        offset: 2311,
        len: 1,
    },
    Decomposition {
        point: 0xFA67,
        offset: 2312,
        len: 1,
    },
    Decomposition {
        point: 0xFA68,
        offset: 2313,
        len: 1,
    },
    Decomposition {
        point: 0xFA69,
        offset: 2314,
        len: 1,
    },
    Decomposition {
        point: 0xFA6A,
        offset: 2315,
        len: 1,
    },
    Decomposition {
        point: 0xFA6B,
        offset: 2316,
        len: 1,
    },
    Decomposition {
        point: 0xFA6C,
        offset: 2317,
        len: 1,
    },
    Decomposition {
        point: 0xFA6D,
        offset: 2318,
        len: 1,
    },
    Decomposition {
        point: 0xFA70,
        offset: 2319,
        len: 1,
    },
    Decomposition {
        point: 0xFA71,
        offset: 2320,
        len: 1,
    },
    Decomposition {
        point: 0xFA72,
        offset: 2321,
        len: 1,
    },
    Decomposition {
        point: 0xFA73,
        offset: 2322,
        len: 1,
    },
    Decomposition {
        point: 0xFA74,
        offset: 2323,
        len: 1,
    },
    Decomposition {
        point: 0xFA75,
        offset: 2324,
        len: 1,
    },
    Decomposition {
        point: 0xFA76,
        offset: 2325,
        len: 1,
    },
    Decomposition {
        point: 0xFA77,
        offset: 2326,
        len: 1,
    },
    Decomposition {
        point: 0xFA78,
        offset: 2327,
        len: 1,
    },
    Decomposition {
        point: 0xFA79,
        offset: 2328,
        len: 1,
    },
    Decomposition {
        point: 0xFA7A,
        offset: 2329,
        len: 1,
    },
    Decomposition {
        point: 0xFA7B,
        offset: 2330,
        len: 1,
    },
    Decomposition {
        point: 0xFA7C,
        offset: 2331,
        len: 1,
    },
    Decomposition {
        point: 0xFA7D,
        offset: 2332,
        len: 1,
    },
    Decomposition {
        point: 0xFA7E,
        offset: 2333,
        len: 1,
    },
    Decomposition {
        point: 0xFA7F,
        offset: 2334,
        len: 1,
    },
    Decomposition {
        point: 0xFA80,
        offset: 2335,
        len: 1,
    },
    Decomposition {
        point: 0xFA81,
        offset: 2336,
        len: 1,
    },
    Decomposition {
        point: 0xFA82,
        offset: 2337,
        len: 1,
    },
    Decomposition {
        point: 0xFA83,
        offset: 2338,
        len: 1,
    },
    Decomposition {
        point: 0xFA84,
        offset: 2339,
        len: 1,
    },
    Decomposition {
        point: 0xFA85,
        offset: 2340,
        len: 1,
    },
    Decomposition {
        point: 0xFA86,
        offset: 2341,
        len: 1,
    },
    Decomposition {
        point: 0xFA87,
        offset: 2342,
        len: 1,
    },
    Decomposition {
        point: 0xFA88,
        offset: 2343,
        len: 1,
    },
    Decomposition {
        point: 0xFA89,
        offset: 2344,
        len: 1,
    },
    Decomposition {
        point: 0xFA8A,
        offset: 2345,
        len: 1,
    },
    Decomposition {
        point: 0xFA8B,
        offset: 2346,
        len: 1,
    },
    Decomposition {
        point: 0xFA8C,
        offset: 2347,
        len: 1,
    },
    Decomposition {
        point: 0xFA8D,
        offset: 2348,
        len: 1,
    },
    Decomposition {
        point: 0xFA8E,
        offset: 2349,
        len: 1,
    },
    Decomposition {
        point: 0xFA8F,
        offset: 2350,
        len: 1,
    },
    Decomposition {
        point: 0xFA90,
        offset: 2351,
        len: 1,
    },
    Decomposition {
        point: 0xFA91,
        offset: 2352,
        len: 1,
    },
    Decomposition {
        point: 0xFA92,
        offset: 2353,
        len: 1,
    },
    Decomposition {
        point: 0xFA93,
        offset: 2354,
        len: 1,
    },
    Decomposition {
        point: 0xFA94,
        offset: 2355,
        len: 1,
    },
    Decomposition {
        point: 0xFA95,
        offset: 2356,
        len: 1,
    },
    Decomposition {
        point: 0xFA96,
        offset: 2357,
        len: 1,
    },
    Decomposition {
        point: 0xFA97,
        offset: 2358,
        len: 1,
    },
    Decomposition {
        point: 0xFA98,
        offset: 2359,
        len: 1,
    },
    Decomposition {
        point: 0xFA99,
        offset: 2360,
        len: 1,
    },
    Decomposition {
        point: 0xFA9A,
        offset: 2361,
        len: 1,
    },
    Decomposition {
        point: 0xFA9B,
        offset: 2362,
        len: 1,
    },
    Decomposition {
        point: 0xFA9C,
        offset: 2363,
        len: 1,
    },
    Decomposition {
        point: 0xFA9D,
        offset: 2364,
        len: 1,
    },
    Decomposition {
        point: 0xFA9E,
        offset: 2365,
        len: 1,
    },
    Decomposition {
        point: 0xFA9F,
        offset: 2366,
        len: 1,
    },
    Decomposition {
        point: 0xFAA0,
        offset: 2367,
        len: 1,
    },
    Decomposition {
        point: 0xFAA1,
        offset: 2368,
        len: 1,
    },
    Decomposition {
        point: 0xFAA2,
        offset: 2369,
        len: 1,
    },
    Decomposition {
        point: 0xFAA3,
        offset: 2370,
        len: 1,
    },
    Decomposition {
        point: 0xFAA4,
        offset: 2371,
        len: 1,
    },
    Decomposition {
        point: 0xFAA5,
        offset: 2372,
        len: 1,
    },
    Decomposition {
        point: 0xFAA6,
        offset: 2373,
        len: 1,
    },
    Decomposition {
        point: 0xFAA7,
        offset: 2374,
        len: 1,
    },
    Decomposition {
        point: 0xFAA8,
        offset: 2375,
        len: 1,
    },
    Decomposition {
        point: 0xFAA9,
        offset: 2376,
        len: 1,
    },
    Decomposition {
        point: 0xFAAA,
        offset: 2377,
        len: 1,
    },
    Decomposition {
        point: 0xFAAB,
        offset: 2378,
        len: 1,
    },
    Decomposition {
        point: 0xFAAC,
        offset: 2379,
        len: 1,
    },
    Decomposition {
        point: 0xFAAD,
        offset: 2380,
        len: 1,
    },
    Decomposition {
        point: 0xFAAE,
        offset: 2381,
        len: 1,
    },
    Decomposition {
        point: 0xFAAF,
        offset: 2382,
        len: 1,
    },
    Decomposition {
        point: 0xFAB0,
        offset: 2383,
        len: 1,
    },
    Decomposition {
        point: 0xFAB1,
        offset: 2384,
        len: 1,
    },
    Decomposition {
        point: 0xFAB2,
        offset: 2385,
        len: 1,
    },
    Decomposition {
        point: 0xFAB3,
        offset: 2386,
        len: 1,
    },
    Decomposition {
        point: 0xFAB4,
        offset: 2387,
        len: 1,
    },
    Decomposition {
        point: 0xFAB5,
        offset: 2388,
        len: 1,
    },
    Decomposition {
        point: 0xFAB6,
        offset: 2389,
        len: 1,
    },
    Decomposition {
        point: 0xFAB7,
        offset: 2390,
        len: 1,
    },
    Decomposition {
        point: 0xFAB8,
        offset: 2391,
        len: 1,
    },
    Decomposition {
        point: 0xFAB9,
        offset: 2392,
        len: 1,
    },
    Decomposition {
        point: 0xFABA,
        offset: 2393,
        len: 1,
    },
    Decomposition {
        point: 0xFABB,
        offset: 2394,
        len: 1,
    },
    Decomposition {
        point: 0xFABC,
        offset: 2395,
        len: 1,
    },
    Decomposition {
        point: 0xFABD,
        offset: 2396,
        len: 1,
    },
    Decomposition {
        point: 0xFABE,
        offset: 2397,
        len: 1,
    },
    Decomposition {
        point: 0xFABF,
        offset: 2398,
        len: 1,
    },
    Decomposition {
        point: 0xFAC0,
        offset: 2399,
        len: 1,
    },
    Decomposition {
        point: 0xFAC1,
        offset: 2400,
        len: 1,
    },
    Decomposition {
        point: 0xFAC2,
        offset: 2401,
        len: 1,
    },
    Decomposition {
        point: 0xFAC3,
        offset: 2402,
        len: 1,
    },
    Decomposition {
        point: 0xFAC4,
        offset: 2403,
        len: 1,
    },
    Decomposition {
        point: 0xFAC5,
        offset: 2404,
        len: 1,
    },
    Decomposition {
        point: 0xFAC6,
        offset: 2405,
        len: 1,
    },
    Decomposition {
        point: 0xFAC7,
        offset: 2406,
        len: 1,
    },
    Decomposition {
        point: 0xFAC8,
        offset: 2407,
        len: 1,
    },
    Decomposition {
        point: 0xFAC9,
        offset: 2408,
        len: 1,
    },
    Decomposition {
        point: 0xFACA,
        offset: 2409,
        len: 1,
    },
    Decomposition {
        point: 0xFACB,
        offset: 2410,
        len: 1,
    },
    Decomposition {
        point: 0xFACC,
        offset: 2411,
        len: 1,
    },
    Decomposition {
        point: 0xFACD,
        offset: 2412,
        len: 1,
    },
    Decomposition {
        point: 0xFACE,
        offset: 2413,
        len: 1,
    },
    Decomposition {
        point: 0xFACF,
        offset: 2414,
        len: 1,
    },
    Decomposition {
        point: 0xFAD0,
        offset: 2415,
        len: 1,
    },
    Decomposition {
        point: 0xFAD1,
        offset: 2416,
        len: 1,
    },
    Decomposition {
        point: 0xFAD2,
        offset: 2417,
        len: 1,
    },
    Decomposition {
        point: 0xFAD3,
        offset: 2418,
        len: 1,
    },
    Decomposition {
        point: 0xFAD4,
        offset: 2419,
        len: 1,
    },
    Decomposition {
        point: 0xFAD5,
        offset: 2420,
        len: 1,
    },
    Decomposition {
        point: 0xFAD6,
        offset: 2421,
        len: 1,
    },
    Decomposition {
        point: 0xFAD7,
        offset: 2422,
        len: 1,
    },
    Decomposition {
        point: 0xFAD8,
        offset: 2423,
        len: 1,
    },
    Decomposition {
        point: 0xFAD9,
        offset: 2424,
        len: 1,
    },
    Decomposition {
        point: 0xFB1D,
        offset: 2425,
        len: 2,
    },
    Decomposition {
        point: 0xFB1F,
        offset: 2427,
        len: 2,
    },
    Decomposition {
        point: 0xFB2A,
        offset: 2429,
        len: 2,
    },
    Decomposition {
        point: 0xFB2B,
        offset: 2431,
        len: 2,
    },
    Decomposition {
        point: 0xFB2C,
        offset: 2433,
        len: 2,
    },
    Decomposition {
        point: 0xFB2D,
        offset: 2435,
        len: 2,
    },
    Decomposition {
        point: 0xFB2E,
        offset: 2437,
        len: 2,
    },
    Decomposition {
        point: 0xFB2F,
        offset: 2439,
        len: 2,
    },
    Decomposition {
        point: 0xFB30,
        offset: 2441,
        len: 2,
    },
    Decomposition {
        point: 0xFB31,
        offset: 2443,
        len: 2,
    },
    Decomposition {
        point: 0xFB32,
        offset: 2445,
        len: 2,
    },
    Decomposition {
        point: 0xFB33,
        offset: 2447,
        len: 2,
    },
    Decomposition {
        point: 0xFB34,
        offset: 2449,
        len: 2,
    },
    Decomposition {
        point: 0xFB35,
        offset: 2451,
        len: 2,
    },
    Decomposition {
        point: 0xFB36,
        offset: 2453,
        len: 2,
    },
    Decomposition {
        point: 0xFB38,
        offset: 2455,
        len: 2,
    },
    Decomposition {
        point: 0xFB39,
        offset: 2457,
        len: 2,
    },
    Decomposition {
        point: 0xFB3A,
        offset: 2459,
        len: 2,
    },
    Decomposition {
        point: 0xFB3B,
        offset: 2461,
        len: 2,
    },
    Decomposition {
        point: 0xFB3C,
        offset: 2463,
        len: 2,
    },
    Decomposition {
        point: 0xFB3E,
        offset: 2465,
        len: 2,
    },
    Decomposition {
        point: 0xFB40,
        offset: 2467,
        len: 2,
    },
    Decomposition {
        point: 0xFB41,
        offset: 2469,
        len: 2,
    },
    Decomposition {
        point: 0xFB43,
        offset: 2471,
        len: 2,
    },
    Decomposition {
        point: 0xFB44,
        offset: 2473,
        len: 2,
    },
    Decomposition {
        point: 0xFB46,
        offset: 2475,
        len: 2,
    },
    Decomposition {
        point: 0xFB47,
        offset: 2477,
        len: 2,
    },
    Decomposition {
        point: 0xFB48,
        offset: 2479,
        len: 2,
    },
    Decomposition {
        point: 0xFB49,
        offset: 2481,
        len: 2,
    },
    Decomposition {
        point: 0xFB4A,
        offset: 2483,
        len: 2,
    },
    Decomposition {
        point: 0xFB4B,
        offset: 2485,
        len: 2,
    },
    Decomposition {
        point: 0xFB4C,
        offset: 2487,
        len: 2,
    },
    Decomposition {
        point: 0xFB4D,
        offset: 2489,
        len: 2,
    },
    Decomposition {
        point: 0xFB4E,
        offset: 2491,
        len: 2,
    },
    Decomposition {
        point: 0x105C9,
        offset: 2493,
        len: 2,
    },
    Decomposition {
        point: 0x105E4,
        offset: 2495,
        len: 2,
    },
    Decomposition {
        point: 0x1109A,
        offset: 2497,
        len: 2,
    },
    Decomposition {
        point: 0x1109C,
        offset: 2499,
        len: 2,
    },
    Decomposition {
        point: 0x110AB,
        offset: 2501,
        len: 2,
    },
    Decomposition {
        point: 0x1112E,
        offset: 2503,
        len: 2,
    },
    Decomposition {
        point: 0x1112F,
        offset: 2505,
        len: 2,
    },
    Decomposition {
        point: 0x1134B,
        offset: 2507,
        len: 2,
    },
    Decomposition {
        point: 0x1134C,
        offset: 2509,
        len: 2,
    },
    Decomposition {
        point: 0x11383,
        offset: 2511,
        len: 2,
    },
    Decomposition {
        point: 0x11385,
        offset: 2513,
        len: 2,
    },
    Decomposition {
        point: 0x1138E,
        offset: 2515,
        len: 2,
    },
    Decomposition {
        point: 0x11391,
        offset: 2517,
        len: 2,
    },
    Decomposition {
        point: 0x113C5,
        offset: 2519,
        len: 2,
    },
    Decomposition {
        point: 0x113C7,
        offset: 2521,
        len: 2,
    },
    Decomposition {
        point: 0x113C8,
        offset: 2523,
        len: 2,
    },
    Decomposition {
        point: 0x114BB,
        offset: 2525,
        len: 2,
    },
    Decomposition {
        point: 0x114BC,
        offset: 2527,
        len: 2,
    },
    Decomposition {
        point: 0x114BE,
        offset: 2529,
        len: 2,
    },
    Decomposition {
        point: 0x115BA,
        offset: 2531,
        len: 2,
    },
    Decomposition {
        point: 0x115BB,
        offset: 2533,
        len: 2,
    },
    Decomposition {
        point: 0x11938,
        offset: 2535,
        len: 2,
    },
    Decomposition {
        point: 0x16121,
        offset: 2537,
        len: 2,
    },
    Decomposition {
        point: 0x16122,
        offset: 2539,
        len: 2,
    },
    Decomposition {
        point: 0x16123,
        offset: 2541,
        len: 2,
    },
    Decomposition {
        point: 0x16124,
        offset: 2543,
        len: 2,
    },
    Decomposition {
        point: 0x16125,
        offset: 2545,
        len: 2,
    },
    Decomposition {
        point: 0x16126,
        offset: 2547,
        len: 2,
    },
    Decomposition {
        point: 0x16127,
        offset: 2549,
        len: 2,
    },
    Decomposition {
        point: 0x16128,
        offset: 2551,
        len: 2,
    },
    Decomposition {
        point: 0x16D68,
        offset: 2553,
        len: 2,
    },
    Decomposition {
        point: 0x16D69,
        offset: 2555,
        len: 2,
    },
    Decomposition {
        point: 0x16D6A,
        offset: 2557,
        len: 2,
    },
    Decomposition {
        point: 0x1D15E,
        offset: 2559,
        len: 2,
    },
    Decomposition {
        point: 0x1D15F,
        offset: 2561,
        len: 2,
    },
    Decomposition {
        point: 0x1D160,
        offset: 2563,
        len: 2,
    },
    Decomposition {
        point: 0x1D161,
        offset: 2565,
        len: 2,
    },
    Decomposition {
        point: 0x1D162,
        offset: 2567,
        len: 2,
    },
    Decomposition {
        point: 0x1D163,
        offset: 2569,
        len: 2,
    },
    Decomposition {
        point: 0x1D164,
        offset: 2571,
        len: 2,
    },
    Decomposition {
        point: 0x1D1BB,
        offset: 2573,
        len: 2,
    },
    Decomposition {
        point: 0x1D1BC,
        offset: 2575,
        len: 2,
    },
    Decomposition {
        point: 0x1D1BD,
        offset: 2577,
        len: 2,
    },
    Decomposition {
        point: 0x1D1BE,
        offset: 2579,
        len: 2,
    },
    Decomposition {
        point: 0x1D1BF,
        offset: 2581,
        len: 2,
    },
    Decomposition {
        point: 0x1D1C0,
        offset: 2583,
        len: 2,
    },
    Decomposition {
        point: 0x2F800,
        offset: 2585,
        len: 1,
    },
    Decomposition {
        point: 0x2F801,
        offset: 2586,
        len: 1,
    },
    Decomposition {
        point: 0x2F802,
        offset: 2587,
        len: 1,
    },
    Decomposition {
        point: 0x2F803,
        offset: 2588,
        len: 1,
    },
    Decomposition {
        point: 0x2F804,
        offset: 2589,
        len: 1,
    },
    Decomposition {
        point: 0x2F805,
        offset: 2590,
        len: 1,
    },
    Decomposition {
        point: 0x2F806,
        offset: 2591,
        len: 1,
    },
    Decomposition {
        point: 0x2F807,
        offset: 2592,
        len: 1,
    },
    Decomposition {
        point: 0x2F808,
        offset: 2593,
        len: 1,
    },
    Decomposition {
        point: 0x2F809,
        offset: 2594,
        len: 1,
    },
    Decomposition {
        point: 0x2F80A,
        offset: 2595,
        len: 1,
    },
    Decomposition {
        point: 0x2F80B,
        offset: 2596,
        len: 1,
    },
    Decomposition {
        point: 0x2F80C,
        offset: 2597,
        len: 1,
    },
    Decomposition {
        point: 0x2F80D,
        offset: 2598,
        len: 1,
    },
    Decomposition {
        point: 0x2F80E,
        offset: 2599,
        len: 1,
    },
    Decomposition {
        point: 0x2F80F,
        offset: 2600,
        len: 1,
    },
    Decomposition {
        point: 0x2F810,
        offset: 2601,
        len: 1,
    },
    Decomposition {
        point: 0x2F811,
        offset: 2602,
        len: 1,
    },
    Decomposition {
        point: 0x2F812,
        offset: 2603,
        len: 1,
    },
    Decomposition {
        point: 0x2F813,
        offset: 2604,
        len: 1,
    },
    Decomposition {
        point: 0x2F814,
        offset: 2605,
        len: 1,
    },
    Decomposition {
        point: 0x2F815,
        offset: 2606,
        len: 1,
    },
    Decomposition {
        point: 0x2F816,
        offset: 2607,
        len: 1,
    },
    Decomposition {
        point: 0x2F817,
        offset: 2608,
        len: 1,
    },
    Decomposition {
        point: 0x2F818,
        offset: 2609,
        len: 1,
    },
    Decomposition {
        point: 0x2F819,
        offset: 2610,
        len: 1,
    },
    Decomposition {
        point: 0x2F81A,
        offset: 2611,
        len: 1,
    },
    Decomposition {
        point: 0x2F81B,
        offset: 2612,
        len: 1,
    },
    Decomposition {
        point: 0x2F81C,
        offset: 2613,
        len: 1,
    },
    Decomposition {
        point: 0x2F81D,
        offset: 2614,
        len: 1,
    },
    Decomposition {
        point: 0x2F81E,
        offset: 2615,
        len: 1,
    },
    Decomposition {
        point: 0x2F81F,
        offset: 2616,
        len: 1,
    },
    Decomposition {
        point: 0x2F820,
        offset: 2617,
        len: 1,
    },
    Decomposition {
        point: 0x2F821,
        offset: 2618,
        len: 1,
    },
    Decomposition {
        point: 0x2F822,
        offset: 2619,
        len: 1,
    },
    Decomposition {
        point: 0x2F823,
        offset: 2620,
        len: 1,
    },
    Decomposition {
        point: 0x2F824,
        offset: 2621,
        len: 1,
    },
    Decomposition {
        point: 0x2F825,
        offset: 2622,
        len: 1,
    },
    Decomposition {
        point: 0x2F826,
        offset: 2623,
        len: 1,
    },
    Decomposition {
        point: 0x2F827,
        offset: 2624,
        len: 1,
    },
    Decomposition {
        point: 0x2F828,
        offset: 2625,
        len: 1,
    },
    Decomposition {
        point: 0x2F829,
        offset: 2626,
        len: 1,
    },
    Decomposition {
        point: 0x2F82A,
        offset: 2627,
        len: 1,
    },
    Decomposition {
        point: 0x2F82B,
        offset: 2628,
        len: 1,
    },
    Decomposition {
        point: 0x2F82C,
        offset: 2629,
        len: 1,
    },
    Decomposition {
        point: 0x2F82D,
        offset: 2630,
        len: 1,
    },
    Decomposition {
        point: 0x2F82E,
        offset: 2631,
        len: 1,
    },
    Decomposition {
        point: 0x2F82F,
        offset: 2632,
        len: 1,
    },
    Decomposition {
        point: 0x2F830,
        offset: 2633,
        len: 1,
    },
    Decomposition {
        point: 0x2F831,
        offset: 2634,
        len: 1,
    },
    Decomposition {
        point: 0x2F832,
        offset: 2635,
        len: 1,
    },
    Decomposition {
        point: 0x2F833,
        offset: 2636,
        len: 1,
    },
    Decomposition {
        point: 0x2F834,
        offset: 2637,
        len: 1,
    },
    Decomposition {
        point: 0x2F835,
        offset: 2638,
        len: 1,
    },
    Decomposition {
        point: 0x2F836,
        offset: 2639,
        len: 1,
    },
    Decomposition {
        point: 0x2F837,
        offset: 2640,
        len: 1,
    },
    Decomposition {
        point: 0x2F838,
        offset: 2641,
        len: 1,
    },
    Decomposition {
        point: 0x2F839,
        offset: 2642,
        len: 1,
    },
    Decomposition {
        point: 0x2F83A,
        offset: 2643,
        len: 1,
    },
    Decomposition {
        point: 0x2F83B,
        offset: 2644,
        len: 1,
    },
    Decomposition {
        point: 0x2F83C,
        offset: 2645,
        len: 1,
    },
    Decomposition {
        point: 0x2F83D,
        offset: 2646,
        len: 1,
    },
    Decomposition {
        point: 0x2F83E,
        offset: 2647,
        len: 1,
    },
    Decomposition {
        point: 0x2F83F,
        offset: 2648,
        len: 1,
    },
    Decomposition {
        point: 0x2F840,
        offset: 2649,
        len: 1,
    },
    Decomposition {
        point: 0x2F841,
        offset: 2650,
        len: 1,
    },
    Decomposition {
        point: 0x2F842,
        offset: 2651,
        len: 1,
    },
    Decomposition {
        point: 0x2F843,
        offset: 2652,
        len: 1,
    },
    Decomposition {
        point: 0x2F844,
        offset: 2653,
        len: 1,
    },
    Decomposition {
        point: 0x2F845,
        offset: 2654,
        len: 1,
    },
    Decomposition {
        point: 0x2F846,
        offset: 2655,
        len: 1,
    },
    Decomposition {
        point: 0x2F847,
        offset: 2656,
        len: 1,
    },
    Decomposition {
        point: 0x2F848,
        offset: 2657,
        len: 1,
    },
    Decomposition {
        point: 0x2F849,
        offset: 2658,
        len: 1,
    },
    Decomposition {
        point: 0x2F84A,
        offset: 2659,
        len: 1,
    },
    Decomposition {
        point: 0x2F84B,
        offset: 2660,
        len: 1,
    },
    Decomposition {
        point: 0x2F84C,
        offset: 2661,
        len: 1,
    },
    Decomposition {
        point: 0x2F84D,
        offset: 2662,
        len: 1,
    },
    Decomposition {
        point: 0x2F84E,
        offset: 2663,
        len: 1,
    },
    Decomposition {
        point: 0x2F84F,
        offset: 2664,
        len: 1,
    },
    Decomposition {
        point: 0x2F850,
        offset: 2665,
        len: 1,
    },
    Decomposition {
        point: 0x2F851,
        offset: 2666,
        len: 1,
    },
    Decomposition {
        point: 0x2F852,
        offset: 2667,
        len: 1,
    },
    Decomposition {
        point: 0x2F853,
        offset: 2668,
        len: 1,
    },
    Decomposition {
        point: 0x2F854,
        offset: 2669,
        len: 1,
    },
    Decomposition {
        point: 0x2F855,
        offset: 2670,
        len: 1,
    },
    Decomposition {
        point: 0x2F856,
        offset: 2671,
        len: 1,
    },
    Decomposition {
        point: 0x2F857,
        offset: 2672,
        len: 1,
    },
    Decomposition {
        point: 0x2F858,
        offset: 2673,
        len: 1,
    },
    Decomposition {
        point: 0x2F859,
        offset: 2674,
        len: 1,
    },
    Decomposition {
        point: 0x2F85A,
        offset: 2675,
        len: 1,
    },
    Decomposition {
        point: 0x2F85B,
        offset: 2676,
        len: 1,
    },
    Decomposition {
        point: 0x2F85C,
        offset: 2677,
        len: 1,
    },
    Decomposition {
        point: 0x2F85D,
        offset: 2678,
        len: 1,
    },
    Decomposition {
        point: 0x2F85E,
        offset: 2679,
        len: 1,
    },
    Decomposition {
        point: 0x2F85F,
        offset: 2680,
        len: 1,
    },
    Decomposition {
        point: 0x2F860,
        offset: 2681,
        len: 1,
    },
    Decomposition {
        point: 0x2F861,
        offset: 2682,
        len: 1,
    },
    Decomposition {
        point: 0x2F862,
        offset: 2683,
        len: 1,
    },
    Decomposition {
        point: 0x2F863,
        offset: 2684,
        len: 1,
    },
    Decomposition {
        point: 0x2F864,
        offset: 2685,
        len: 1,
    },
    Decomposition {
        point: 0x2F865,
        offset: 2686,
        len: 1,
    },
    Decomposition {
        point: 0x2F866,
        offset: 2687,
        len: 1,
    },
    Decomposition {
        point: 0x2F867,
        offset: 2688,
        len: 1,
    },
    Decomposition {
        point: 0x2F868,
        offset: 2689,
        len: 1,
    },
    Decomposition {
        point: 0x2F869,
        offset: 2690,
        len: 1,
    },
    Decomposition {
        point: 0x2F86A,
        offset: 2691,
        len: 1,
    },
    Decomposition {
        point: 0x2F86B,
        offset: 2692,
        len: 1,
    },
    Decomposition {
        point: 0x2F86C,
        offset: 2693,
        len: 1,
    },
    Decomposition {
        point: 0x2F86D,
        offset: 2694,
        len: 1,
    },
    Decomposition {
        point: 0x2F86E,
        offset: 2695,
        len: 1,
    },
    Decomposition {
        point: 0x2F86F,
        offset: 2696,
        len: 1,
    },
    Decomposition {
        point: 0x2F870,
        offset: 2697,
        len: 1,
    },
    Decomposition {
        point: 0x2F871,
        offset: 2698,
        len: 1,
    },
    Decomposition {
        point: 0x2F872,
        offset: 2699,
        len: 1,
    },
    Decomposition {
        point: 0x2F873,
        offset: 2700,
        len: 1,
    },
    Decomposition {
        point: 0x2F874,
        offset: 2701,
        len: 1,
    },
    Decomposition {
        point: 0x2F875,
        offset: 2702,
        len: 1,
    },
    Decomposition {
        point: 0x2F876,
        offset: 2703,
        len: 1,
    },
    Decomposition {
        point: 0x2F877,
        offset: 2704,
        len: 1,
    },
    Decomposition {
        point: 0x2F878,
        offset: 2705,
        len: 1,
    },
    Decomposition {
        point: 0x2F879,
        offset: 2706,
        len: 1,
    },
    Decomposition {
        point: 0x2F87A,
        offset: 2707,
        len: 1,
    },
    Decomposition {
        point: 0x2F87B,
        offset: 2708,
        len: 1,
    },
    Decomposition {
        point: 0x2F87C,
        offset: 2709,
        len: 1,
    },
    Decomposition {
        point: 0x2F87D,
        offset: 2710,
        len: 1,
    },
    Decomposition {
        point: 0x2F87E,
        offset: 2711,
        len: 1,
    },
    Decomposition {
        point: 0x2F87F,
        offset: 2712,
        len: 1,
    },
    Decomposition {
        point: 0x2F880,
        offset: 2713,
        len: 1,
    },
    Decomposition {
        point: 0x2F881,
        offset: 2714,
        len: 1,
    },
    Decomposition {
        point: 0x2F882,
        offset: 2715,
        len: 1,
    },
    Decomposition {
        point: 0x2F883,
        offset: 2716,
        len: 1,
    },
    Decomposition {
        point: 0x2F884,
        offset: 2717,
        len: 1,
    },
    Decomposition {
        point: 0x2F885,
        offset: 2718,
        len: 1,
    },
    Decomposition {
        point: 0x2F886,
        offset: 2719,
        len: 1,
    },
    Decomposition {
        point: 0x2F887,
        offset: 2720,
        len: 1,
    },
    Decomposition {
        point: 0x2F888,
        offset: 2721,
        len: 1,
    },
    Decomposition {
        point: 0x2F889,
        offset: 2722,
        len: 1,
    },
    Decomposition {
        point: 0x2F88A,
        offset: 2723,
        len: 1,
    },
    Decomposition {
        point: 0x2F88B,
        offset: 2724,
        len: 1,
    },
    Decomposition {
        point: 0x2F88C,
        offset: 2725,
        len: 1,
    },
    Decomposition {
        point: 0x2F88D,
        offset: 2726,
        len: 1,
    },
    Decomposition {
        point: 0x2F88E,
        offset: 2727,
        len: 1,
    },
    Decomposition {
        point: 0x2F88F,
        offset: 2728,
        len: 1,
    },
    Decomposition {
        point: 0x2F890,
        offset: 2729,
        len: 1,
    },
    Decomposition {
        point: 0x2F891,
        offset: 2730,
        len: 1,
    },
    Decomposition {
        point: 0x2F892,
        offset: 2731,
        len: 1,
    },
    Decomposition {
        point: 0x2F893,
        offset: 2732,
        len: 1,
    },
    Decomposition {
        point: 0x2F894,
        offset: 2733,
        len: 1,
    },
    Decomposition {
        point: 0x2F895,
        offset: 2734,
        len: 1,
    },
    Decomposition {
        point: 0x2F896,
        offset: 2735,
        len: 1,
    },
    Decomposition {
        point: 0x2F897,
        offset: 2736,
        len: 1,
    },
    Decomposition {
        point: 0x2F898,
        offset: 2737,
        len: 1,
    },
    Decomposition {
        point: 0x2F899,
        offset: 2738,
        len: 1,
    },
    Decomposition {
        point: 0x2F89A,
        offset: 2739,
        len: 1,
    },
    Decomposition {
        point: 0x2F89B,
        offset: 2740,
        len: 1,
    },
    Decomposition {
        point: 0x2F89C,
        offset: 2741,
        len: 1,
    },
    Decomposition {
        point: 0x2F89D,
        offset: 2742,
        len: 1,
    },
    Decomposition {
        point: 0x2F89E,
        offset: 2743,
        len: 1,
    },
    Decomposition {
        point: 0x2F89F,
        offset: 2744,
        len: 1,
    },
    Decomposition {
        point: 0x2F8A0,
        offset: 2745,
        len: 1,
    },
    Decomposition {
        point: 0x2F8A1,
        offset: 2746,
        len: 1,
    },
    Decomposition {
        point: 0x2F8A2,
        offset: 2747,
        len: 1,
    },
    Decomposition {
        point: 0x2F8A3,
        offset: 2748,
        len: 1,
    },
    Decomposition {
        point: 0x2F8A4,
        offset: 2749,
        len: 1,
    },
    Decomposition {
        point: 0x2F8A5,
        offset: 2750,
        len: 1,
    },
    Decomposition {
        point: 0x2F8A6,
        offset: 2751,
        len: 1,
    },
    Decomposition {
        point: 0x2F8A7,
        offset: 2752,
        len: 1,
    },
    Decomposition {
        point: 0x2F8A8,
        offset: 2753,
        len: 1,
    },
    Decomposition {
        point: 0x2F8A9,
        offset: 2754,
        len: 1,
    },
    Decomposition {
        point: 0x2F8AA,
        offset: 2755,
        len: 1,
    },
    Decomposition {
        point: 0x2F8AB,
        offset: 2756,
        len: 1,
    },
    Decomposition {
        point: 0x2F8AC,
        offset: 2757,
        len: 1,
    },
    Decomposition {
        point: 0x2F8AD,
        offset: 2758,
        len: 1,
    },
    Decomposition {
        point: 0x2F8AE,
        offset: 2759,
        len: 1,
    },
    Decomposition {
        point: 0x2F8AF,
        offset: 2760,
        len: 1,
    },
    Decomposition {
        point: 0x2F8B0,
        offset: 2761,
        len: 1,
    },
    Decomposition {
        point: 0x2F8B1,
        offset: 2762,
        len: 1,
    },
    Decomposition {
        point: 0x2F8B2,
        offset: 2763,
        len: 1,
    },
    Decomposition {
        point: 0x2F8B3,
        offset: 2764,
        len: 1,
    },
    Decomposition {
        point: 0x2F8B4,
        offset: 2765,
        len: 1,
    },
    Decomposition {
        point: 0x2F8B5,
        offset: 2766,
        len: 1,
    },
    Decomposition {
        point: 0x2F8B6,
        offset: 2767,
        len: 1,
    },
    Decomposition {
        point: 0x2F8B7,
        offset: 2768,
        len: 1,
    },
    Decomposition {
        point: 0x2F8B8,
        offset: 2769,
        len: 1,
    },
    Decomposition {
        point: 0x2F8B9,
        offset: 2770,
        len: 1,
    },
    Decomposition {
        point: 0x2F8BA,
        offset: 2771,
        len: 1,
    },
    Decomposition {
        point: 0x2F8BB,
        offset: 2772,
        len: 1,
    },
    Decomposition {
        point: 0x2F8BC,
        offset: 2773,
        len: 1,
    },
    Decomposition {
        point: 0x2F8BD,
        offset: 2774,
        len: 1,
    },
    Decomposition {
        point: 0x2F8BE,
        offset: 2775,
        len: 1,
    },
    Decomposition {
        point: 0x2F8BF,
        offset: 2776,
        len: 1,
    },
    Decomposition {
        point: 0x2F8C0,
        offset: 2777,
        len: 1,
    },
    Decomposition {
        point: 0x2F8C1,
        offset: 2778,
        len: 1,
    },
    Decomposition {
        point: 0x2F8C2,
        offset: 2779,
        len: 1,
    },
    Decomposition {
        point: 0x2F8C3,
        offset: 2780,
        len: 1,
    },
    Decomposition {
        point: 0x2F8C4,
        offset: 2781,
        len: 1,
    },
    Decomposition {
        point: 0x2F8C5,
        offset: 2782,
        len: 1,
    },
    Decomposition {
        point: 0x2F8C6,
        offset: 2783,
        len: 1,
    },
    Decomposition {
        point: 0x2F8C7,
        offset: 2784,
        len: 1,
    },
    Decomposition {
        point: 0x2F8C8,
        offset: 2785,
        len: 1,
    },
    Decomposition {
        point: 0x2F8C9,
        offset: 2786,
        len: 1,
    },
    Decomposition {
        point: 0x2F8CA,
        offset: 2787,
        len: 1,
    },
    Decomposition {
        point: 0x2F8CB,
        offset: 2788,
        len: 1,
    },
    Decomposition {
        point: 0x2F8CC,
        offset: 2789,
        len: 1,
    },
    Decomposition {
        point: 0x2F8CD,
        offset: 2790,
        len: 1,
    },
    Decomposition {
        point: 0x2F8CE,
        offset: 2791,
        len: 1,
    },
    Decomposition {
        point: 0x2F8CF,
        offset: 2792,
        len: 1,
    },
    Decomposition {
        point: 0x2F8D0,
        offset: 2793,
        len: 1,
    },
    Decomposition {
        point: 0x2F8D1,
        offset: 2794,
        len: 1,
    },
    Decomposition {
        point: 0x2F8D2,
        offset: 2795,
        len: 1,
    },
    Decomposition {
        point: 0x2F8D3,
        offset: 2796,
        len: 1,
    },
    Decomposition {
        point: 0x2F8D4,
        offset: 2797,
        len: 1,
    },
    Decomposition {
        point: 0x2F8D5,
        offset: 2798,
        len: 1,
    },
    Decomposition {
        point: 0x2F8D6,
        offset: 2799,
        len: 1,
    },
    Decomposition {
        point: 0x2F8D7,
        offset: 2800,
        len: 1,
    },
    Decomposition {
        point: 0x2F8D8,
        offset: 2801,
        len: 1,
    },
    Decomposition {
        point: 0x2F8D9,
        offset: 2802,
        len: 1,
    },
    Decomposition {
        point: 0x2F8DA,
        offset: 2803,
        len: 1,
    },
    Decomposition {
        point: 0x2F8DB,
        offset: 2804,
        len: 1,
    },
    Decomposition {
        point: 0x2F8DC,
        offset: 2805,
        len: 1,
    },
    Decomposition {
        point: 0x2F8DD,
        offset: 2806,
        len: 1,
    },
    Decomposition {
        point: 0x2F8DE,
        offset: 2807,
        len: 1,
    },
    Decomposition {
        point: 0x2F8DF,
        offset: 2808,
        len: 1,
    },
    Decomposition {
        point: 0x2F8E0,
        offset: 2809,
        len: 1,
    },
    Decomposition {
        point: 0x2F8E1,
        offset: 2810,
        len: 1,
    },
    Decomposition {
        point: 0x2F8E2,
        offset: 2811,
        len: 1,
    },
    Decomposition {
        point: 0x2F8E3,
        offset: 2812,
        len: 1,
    },
    Decomposition {
        point: 0x2F8E4,
        offset: 2813,
        len: 1,
    },
    Decomposition {
        point: 0x2F8E5,
        offset: 2814,
        len: 1,
    },
    Decomposition {
        point: 0x2F8E6,
        offset: 2815,
        len: 1,
    },
    Decomposition {
        point: 0x2F8E7,
        offset: 2816,
        len: 1,
    },
    Decomposition {
        point: 0x2F8E8,
        offset: 2817,
        len: 1,
    },
    Decomposition {
        point: 0x2F8E9,
        offset: 2818,
        len: 1,
    },
    Decomposition {
        point: 0x2F8EA,
        offset: 2819,
        len: 1,
    },
    Decomposition {
        point: 0x2F8EB,
        offset: 2820,
        len: 1,
    },
    Decomposition {
        point: 0x2F8EC,
        offset: 2821,
        len: 1,
    },
    Decomposition {
        point: 0x2F8ED,
        offset: 2822,
        len: 1,
    },
    Decomposition {
        point: 0x2F8EE,
        offset: 2823,
        len: 1,
    },
    Decomposition {
        point: 0x2F8EF,
        offset: 2824,
        len: 1,
    },
    Decomposition {
        point: 0x2F8F0,
        offset: 2825,
        len: 1,
    },
    Decomposition {
        point: 0x2F8F1,
        offset: 2826,
        len: 1,
    },
    Decomposition {
        point: 0x2F8F2,
        offset: 2827,
        len: 1,
    },
    Decomposition {
        point: 0x2F8F3,
        offset: 2828,
        len: 1,
    },
    Decomposition {
        point: 0x2F8F4,
        offset: 2829,
        len: 1,
    },
    Decomposition {
        point: 0x2F8F5,
        offset: 2830,
        len: 1,
    },
    Decomposition {
        point: 0x2F8F6,
        offset: 2831,
        len: 1,
    },
    Decomposition {
        point: 0x2F8F7,
        offset: 2832,
        len: 1,
    },
    Decomposition {
        point: 0x2F8F8,
        offset: 2833,
        len: 1,
    },
    Decomposition {
        point: 0x2F8F9,
        offset: 2834,
        len: 1,
    },
    Decomposition {
        point: 0x2F8FA,
        offset: 2835,
        len: 1,
    },
    Decomposition {
        point: 0x2F8FB,
        offset: 2836,
        len: 1,
    },
    Decomposition {
        point: 0x2F8FC,
        offset: 2837,
        len: 1,
    },
    Decomposition {
        point: 0x2F8FD,
        offset: 2838,
        len: 1,
    },
    Decomposition {
        point: 0x2F8FE,
        offset: 2839,
        len: 1,
    },
    Decomposition {
        point: 0x2F8FF,
        offset: 2840,
        len: 1,
    },
    Decomposition {
        point: 0x2F900,
        offset: 2841,
        len: 1,
    },
    Decomposition {
        point: 0x2F901,
        offset: 2842,
        len: 1,
    },
    Decomposition {
        point: 0x2F902,
        offset: 2843,
        len: 1,
    },
    Decomposition {
        point: 0x2F903,
        offset: 2844,
        len: 1,
    },
    Decomposition {
        point: 0x2F904,
        offset: 2845,
        len: 1,
    },
    Decomposition {
        point: 0x2F905,
        offset: 2846,
        len: 1,
    },
    Decomposition {
        point: 0x2F906,
        offset: 2847,
        len: 1,
    },
    Decomposition {
        point: 0x2F907,
        offset: 2848,
        len: 1,
    },
    Decomposition {
        point: 0x2F908,
        offset: 2849,
        len: 1,
    },
    Decomposition {
        point: 0x2F909,
        offset: 2850,
        len: 1,
    },
    Decomposition {
        point: 0x2F90A,
        offset: 2851,
        len: 1,
    },
    Decomposition {
        point: 0x2F90B,
        offset: 2852,
        len: 1,
    },
    Decomposition {
        point: 0x2F90C,
        offset: 2853,
        len: 1,
    },
    Decomposition {
        point: 0x2F90D,
        offset: 2854,
        len: 1,
    },
    Decomposition {
        point: 0x2F90E,
        offset: 2855,
        len: 1,
    },
    Decomposition {
        point: 0x2F90F,
        offset: 2856,
        len: 1,
    },
    Decomposition {
        point: 0x2F910,
        offset: 2857,
        len: 1,
    },
    Decomposition {
        point: 0x2F911,
        offset: 2858,
        len: 1,
    },
    Decomposition {
        point: 0x2F912,
        offset: 2859,
        len: 1,
    },
    Decomposition {
        point: 0x2F913,
        offset: 2860,
        len: 1,
    },
    Decomposition {
        point: 0x2F914,
        offset: 2861,
        len: 1,
    },
    Decomposition {
        point: 0x2F915,
        offset: 2862,
        len: 1,
    },
    Decomposition {
        point: 0x2F916,
        offset: 2863,
        len: 1,
    },
    Decomposition {
        point: 0x2F917,
        offset: 2864,
        len: 1,
    },
    Decomposition {
        point: 0x2F918,
        offset: 2865,
        len: 1,
    },
    Decomposition {
        point: 0x2F919,
        offset: 2866,
        len: 1,
    },
    Decomposition {
        point: 0x2F91A,
        offset: 2867,
        len: 1,
    },
    Decomposition {
        point: 0x2F91B,
        offset: 2868,
        len: 1,
    },
    Decomposition {
        point: 0x2F91C,
        offset: 2869,
        len: 1,
    },
    Decomposition {
        point: 0x2F91D,
        offset: 2870,
        len: 1,
    },
    Decomposition {
        point: 0x2F91E,
        offset: 2871,
        len: 1,
    },
    Decomposition {
        point: 0x2F91F,
        offset: 2872,
        len: 1,
    },
    Decomposition {
        point: 0x2F920,
        offset: 2873,
        len: 1,
    },
    Decomposition {
        point: 0x2F921,
        offset: 2874,
        len: 1,
    },
    Decomposition {
        point: 0x2F922,
        offset: 2875,
        len: 1,
    },
    Decomposition {
        point: 0x2F923,
        offset: 2876,
        len: 1,
    },
    Decomposition {
        point: 0x2F924,
        offset: 2877,
        len: 1,
    },
    Decomposition {
        point: 0x2F925,
        offset: 2878,
        len: 1,
    },
    Decomposition {
        point: 0x2F926,
        offset: 2879,
        len: 1,
    },
    Decomposition {
        point: 0x2F927,
        offset: 2880,
        len: 1,
    },
    Decomposition {
        point: 0x2F928,
        offset: 2881,
        len: 1,
    },
    Decomposition {
        point: 0x2F929,
        offset: 2882,
        len: 1,
    },
    Decomposition {
        point: 0x2F92A,
        offset: 2883,
        len: 1,
    },
    Decomposition {
        point: 0x2F92B,
        offset: 2884,
        len: 1,
    },
    Decomposition {
        point: 0x2F92C,
        offset: 2885,
        len: 1,
    },
    Decomposition {
        point: 0x2F92D,
        offset: 2886,
        len: 1,
    },
    Decomposition {
        point: 0x2F92E,
        offset: 2887,
        len: 1,
    },
    Decomposition {
        point: 0x2F92F,
        offset: 2888,
        len: 1,
    },
    Decomposition {
        point: 0x2F930,
        offset: 2889,
        len: 1,
    },
    Decomposition {
        point: 0x2F931,
        offset: 2890,
        len: 1,
    },
    Decomposition {
        point: 0x2F932,
        offset: 2891,
        len: 1,
    },
    Decomposition {
        point: 0x2F933,
        offset: 2892,
        len: 1,
    },
    Decomposition {
        point: 0x2F934,
        offset: 2893,
        len: 1,
    },
    Decomposition {
        point: 0x2F935,
        offset: 2894,
        len: 1,
    },
    Decomposition {
        point: 0x2F936,
        offset: 2895,
        len: 1,
    },
    Decomposition {
        point: 0x2F937,
        offset: 2896,
        len: 1,
    },
    Decomposition {
        point: 0x2F938,
        offset: 2897,
        len: 1,
    },
    Decomposition {
        point: 0x2F939,
        offset: 2898,
        len: 1,
    },
    Decomposition {
        point: 0x2F93A,
        offset: 2899,
        len: 1,
    },
    Decomposition {
        point: 0x2F93B,
        offset: 2900,
        len: 1,
    },
    Decomposition {
        point: 0x2F93C,
        offset: 2901,
        len: 1,
    },
    Decomposition {
        point: 0x2F93D,
        offset: 2902,
        len: 1,
    },
    Decomposition {
        point: 0x2F93E,
        offset: 2903,
        len: 1,
    },
    Decomposition {
        point: 0x2F93F,
        offset: 2904,
        len: 1,
    },
    Decomposition {
        point: 0x2F940,
        offset: 2905,
        len: 1,
    },
    Decomposition {
        point: 0x2F941,
        offset: 2906,
        len: 1,
    },
    Decomposition {
        point: 0x2F942,
        offset: 2907,
        len: 1,
    },
    Decomposition {
        point: 0x2F943,
        offset: 2908,
        len: 1,
    },
    Decomposition {
        point: 0x2F944,
        offset: 2909,
        len: 1,
    },
    Decomposition {
        point: 0x2F945,
        offset: 2910,
        len: 1,
    },
    Decomposition {
        point: 0x2F946,
        offset: 2911,
        len: 1,
    },
    Decomposition {
        point: 0x2F947,
        offset: 2912,
        len: 1,
    },
    Decomposition {
        point: 0x2F948,
        offset: 2913,
        len: 1,
    },
    Decomposition {
        point: 0x2F949,
        offset: 2914,
        len: 1,
    },
    Decomposition {
        point: 0x2F94A,
        offset: 2915,
        len: 1,
    },
    Decomposition {
        point: 0x2F94B,
        offset: 2916,
        len: 1,
    },
    Decomposition {
        point: 0x2F94C,
        offset: 2917,
        len: 1,
    },
    Decomposition {
        point: 0x2F94D,
        offset: 2918,
        len: 1,
    },
    Decomposition {
        point: 0x2F94E,
        offset: 2919,
        len: 1,
    },
    Decomposition {
        point: 0x2F94F,
        offset: 2920,
        len: 1,
    },
    Decomposition {
        point: 0x2F950,
        offset: 2921,
        len: 1,
    },
    Decomposition {
        point: 0x2F951,
        offset: 2922,
        len: 1,
    },
    Decomposition {
        point: 0x2F952,
        offset: 2923,
        len: 1,
    },
    Decomposition {
        point: 0x2F953,
        offset: 2924,
        len: 1,
    },
    Decomposition {
        point: 0x2F954,
        offset: 2925,
        len: 1,
    },
    Decomposition {
        point: 0x2F955,
        offset: 2926,
        len: 1,
    },
    Decomposition {
        point: 0x2F956,
        offset: 2927,
        len: 1,
    },
    Decomposition {
        point: 0x2F957,
        offset: 2928,
        len: 1,
    },
    Decomposition {
        point: 0x2F958,
        offset: 2929,
        len: 1,
    },
    Decomposition {
        point: 0x2F959,
        offset: 2930,
        len: 1,
    },
    Decomposition {
        point: 0x2F95A,
        offset: 2931,
        len: 1,
    },
    Decomposition {
        point: 0x2F95B,
        offset: 2932,
        len: 1,
    },
    Decomposition {
        point: 0x2F95C,
        offset: 2933,
        len: 1,
    },
    Decomposition {
        point: 0x2F95D,
        offset: 2934,
        len: 1,
    },
    Decomposition {
        point: 0x2F95E,
        offset: 2935,
        len: 1,
    },
    Decomposition {
        point: 0x2F95F,
        offset: 2936,
        len: 1,
    },
    Decomposition {
        point: 0x2F960,
        offset: 2937,
        len: 1,
    },
    Decomposition {
        point: 0x2F961,
        offset: 2938,
        len: 1,
    },
    Decomposition {
        point: 0x2F962,
        offset: 2939,
        len: 1,
    },
    Decomposition {
        point: 0x2F963,
        offset: 2940,
        len: 1,
    },
    Decomposition {
        point: 0x2F964,
        offset: 2941,
        len: 1,
    },
    Decomposition {
        point: 0x2F965,
        offset: 2942,
        len: 1,
    },
    Decomposition {
        point: 0x2F966,
        offset: 2943,
        len: 1,
    },
    Decomposition {
        point: 0x2F967,
        offset: 2944,
        len: 1,
    },
    Decomposition {
        point: 0x2F968,
        offset: 2945,
        len: 1,
    },
    Decomposition {
        point: 0x2F969,
        offset: 2946,
        len: 1,
    },
    Decomposition {
        point: 0x2F96A,
        offset: 2947,
        len: 1,
    },
    Decomposition {
        point: 0x2F96B,
        offset: 2948,
        len: 1,
    },
    Decomposition {
        point: 0x2F96C,
        offset: 2949,
        len: 1,
    },
    Decomposition {
        point: 0x2F96D,
        offset: 2950,
        len: 1,
    },
    Decomposition {
        point: 0x2F96E,
        offset: 2951,
        len: 1,
    },
    Decomposition {
        point: 0x2F96F,
        offset: 2952,
        len: 1,
    },
    Decomposition {
        point: 0x2F970,
        offset: 2953,
        len: 1,
    },
    Decomposition {
        point: 0x2F971,
        offset: 2954,
        len: 1,
    },
    Decomposition {
        point: 0x2F972,
        offset: 2955,
        len: 1,
    },
    Decomposition {
        point: 0x2F973,
        offset: 2956,
        len: 1,
    },
    Decomposition {
        point: 0x2F974,
        offset: 2957,
        len: 1,
    },
    Decomposition {
        point: 0x2F975,
        offset: 2958,
        len: 1,
    },
    Decomposition {
        point: 0x2F976,
        offset: 2959,
        len: 1,
    },
    Decomposition {
        point: 0x2F977,
        offset: 2960,
        len: 1,
    },
    Decomposition {
        point: 0x2F978,
        offset: 2961,
        len: 1,
    },
    Decomposition {
        point: 0x2F979,
        offset: 2962,
        len: 1,
    },
    Decomposition {
        point: 0x2F97A,
        offset: 2963,
        len: 1,
    },
    Decomposition {
        point: 0x2F97B,
        offset: 2964,
        len: 1,
    },
    Decomposition {
        point: 0x2F97C,
        offset: 2965,
        len: 1,
    },
    Decomposition {
        point: 0x2F97D,
        offset: 2966,
        len: 1,
    },
    Decomposition {
        point: 0x2F97E,
        offset: 2967,
        len: 1,
    },
    Decomposition {
        point: 0x2F97F,
        offset: 2968,
        len: 1,
    },
    Decomposition {
        point: 0x2F980,
        offset: 2969,
        len: 1,
    },
    Decomposition {
        point: 0x2F981,
        offset: 2970,
        len: 1,
    },
    Decomposition {
        point: 0x2F982,
        offset: 2971,
        len: 1,
    },
    Decomposition {
        point: 0x2F983,
        offset: 2972,
        len: 1,
    },
    Decomposition {
        point: 0x2F984,
        offset: 2973,
        len: 1,
    },
    Decomposition {
        point: 0x2F985,
        offset: 2974,
        len: 1,
    },
    Decomposition {
        point: 0x2F986,
        offset: 2975,
        len: 1,
    },
    Decomposition {
        point: 0x2F987,
        offset: 2976,
        len: 1,
    },
    Decomposition {
        point: 0x2F988,
        offset: 2977,
        len: 1,
    },
    Decomposition {
        point: 0x2F989,
        offset: 2978,
        len: 1,
    },
    Decomposition {
        point: 0x2F98A,
        offset: 2979,
        len: 1,
    },
    Decomposition {
        point: 0x2F98B,
        offset: 2980,
        len: 1,
    },
    Decomposition {
        point: 0x2F98C,
        offset: 2981,
        len: 1,
    },
    Decomposition {
        point: 0x2F98D,
        offset: 2982,
        len: 1,
    },
    Decomposition {
        point: 0x2F98E,
        offset: 2983,
        len: 1,
    },
    Decomposition {
        point: 0x2F98F,
        offset: 2984,
        len: 1,
    },
    Decomposition {
        point: 0x2F990,
        offset: 2985,
        len: 1,
    },
    Decomposition {
        point: 0x2F991,
        offset: 2986,
        len: 1,
    },
    Decomposition {
        point: 0x2F992,
        offset: 2987,
        len: 1,
    },
    Decomposition {
        point: 0x2F993,
        offset: 2988,
        len: 1,
    },
    Decomposition {
        point: 0x2F994,
        offset: 2989,
        len: 1,
    },
    Decomposition {
        point: 0x2F995,
        offset: 2990,
        len: 1,
    },
    Decomposition {
        point: 0x2F996,
        offset: 2991,
        len: 1,
    },
    Decomposition {
        point: 0x2F997,
        offset: 2992,
        len: 1,
    },
    Decomposition {
        point: 0x2F998,
        offset: 2993,
        len: 1,
    },
    Decomposition {
        point: 0x2F999,
        offset: 2994,
        len: 1,
    },
    Decomposition {
        point: 0x2F99A,
        offset: 2995,
        len: 1,
    },
    Decomposition {
        point: 0x2F99B,
        offset: 2996,
        len: 1,
    },
    Decomposition {
        point: 0x2F99C,
        offset: 2997,
        len: 1,
    },
    Decomposition {
        point: 0x2F99D,
        offset: 2998,
        len: 1,
    },
    Decomposition {
        point: 0x2F99E,
        offset: 2999,
        len: 1,
    },
    Decomposition {
        point: 0x2F99F,
        offset: 3000,
        len: 1,
    },
    Decomposition {
        point: 0x2F9A0,
        offset: 3001,
        len: 1,
    },
    Decomposition {
        point: 0x2F9A1,
        offset: 3002,
        len: 1,
    },
    Decomposition {
        point: 0x2F9A2,
        offset: 3003,
        len: 1,
    },
    Decomposition {
        point: 0x2F9A3,
        offset: 3004,
        len: 1,
    },
    Decomposition {
        point: 0x2F9A4,
        offset: 3005,
        len: 1,
    },
    Decomposition {
        point: 0x2F9A5,
        offset: 3006,
        len: 1,
    },
    Decomposition {
        point: 0x2F9A6,
        offset: 3007,
        len: 1,
    },
    Decomposition {
        point: 0x2F9A7,
        offset: 3008,
        len: 1,
    },
    Decomposition {
        point: 0x2F9A8,
        offset: 3009,
        len: 1,
    },
    Decomposition {
        point: 0x2F9A9,
        offset: 3010,
        len: 1,
    },
    Decomposition {
        point: 0x2F9AA,
        offset: 3011,
        len: 1,
    },
    Decomposition {
        point: 0x2F9AB,
        offset: 3012,
        len: 1,
    },
    Decomposition {
        point: 0x2F9AC,
        offset: 3013,
        len: 1,
    },
    Decomposition {
        point: 0x2F9AD,
        offset: 3014,
        len: 1,
    },
    Decomposition {
        point: 0x2F9AE,
        offset: 3015,
        len: 1,
    },
    Decomposition {
        point: 0x2F9AF,
        offset: 3016,
        len: 1,
    },
    Decomposition {
        point: 0x2F9B0,
        offset: 3017,
        len: 1,
    },
    Decomposition {
        point: 0x2F9B1,
        offset: 3018,
        len: 1,
    },
    Decomposition {
        point: 0x2F9B2,
        offset: 3019,
        len: 1,
    },
    Decomposition {
        point: 0x2F9B3,
        offset: 3020,
        len: 1,
    },
    Decomposition {
        point: 0x2F9B4,
        offset: 3021,
        len: 1,
    },
    Decomposition {
        point: 0x2F9B5,
        offset: 3022,
        len: 1,
    },
    Decomposition {
        point: 0x2F9B6,
        offset: 3023,
        len: 1,
    },
    Decomposition {
        point: 0x2F9B7,
        offset: 3024,
        len: 1,
    },
    Decomposition {
        point: 0x2F9B8,
        offset: 3025,
        len: 1,
    },
    Decomposition {
        point: 0x2F9B9,
        offset: 3026,
        len: 1,
    },
    Decomposition {
        point: 0x2F9BA,
        offset: 3027,
        len: 1,
    },
    Decomposition {
        point: 0x2F9BB,
        offset: 3028,
        len: 1,
    },
    Decomposition {
        point: 0x2F9BC,
        offset: 3029,
        len: 1,
    },
    Decomposition {
        point: 0x2F9BD,
        offset: 3030,
        len: 1,
    },
    Decomposition {
        point: 0x2F9BE,
        offset: 3031,
        len: 1,
    },
    Decomposition {
        point: 0x2F9BF,
        offset: 3032,
        len: 1,
    },
    Decomposition {
        point: 0x2F9C0,
        offset: 3033,
        len: 1,
    },
    Decomposition {
        point: 0x2F9C1,
        offset: 3034,
        len: 1,
    },
    Decomposition {
        point: 0x2F9C2,
        offset: 3035,
        len: 1,
    },
    Decomposition {
        point: 0x2F9C3,
        offset: 3036,
        len: 1,
    },
    Decomposition {
        point: 0x2F9C4,
        offset: 3037,
        len: 1,
    },
    Decomposition {
        point: 0x2F9C5,
        offset: 3038,
        len: 1,
    },
    Decomposition {
        point: 0x2F9C6,
        offset: 3039,
        len: 1,
    },
    Decomposition {
        point: 0x2F9C7,
        offset: 3040,
        len: 1,
    },
    Decomposition {
        point: 0x2F9C8,
        offset: 3041,
        len: 1,
    },
    Decomposition {
        point: 0x2F9C9,
        offset: 3042,
        len: 1,
    },
    Decomposition {
        point: 0x2F9CA,
        offset: 3043,
        len: 1,
    },
    Decomposition {
        point: 0x2F9CB,
        offset: 3044,
        len: 1,
    },
    Decomposition {
        point: 0x2F9CC,
        offset: 3045,
        len: 1,
    },
    Decomposition {
        point: 0x2F9CD,
        offset: 3046,
        len: 1,
    },
    Decomposition {
        point: 0x2F9CE,
        offset: 3047,
        len: 1,
    },
    Decomposition {
        point: 0x2F9CF,
        offset: 3048,
        len: 1,
    },
    Decomposition {
        point: 0x2F9D0,
        offset: 3049,
        len: 1,
    },
    Decomposition {
        point: 0x2F9D1,
        offset: 3050,
        len: 1,
    },
    Decomposition {
        point: 0x2F9D2,
        offset: 3051,
        len: 1,
    },
    Decomposition {
        point: 0x2F9D3,
        offset: 3052,
        len: 1,
    },
    Decomposition {
        point: 0x2F9D4,
        offset: 3053,
        len: 1,
    },
    Decomposition {
        point: 0x2F9D5,
        offset: 3054,
        len: 1,
    },
    Decomposition {
        point: 0x2F9D6,
        offset: 3055,
        len: 1,
    },
    Decomposition {
        point: 0x2F9D7,
        offset: 3056,
        len: 1,
    },
    Decomposition {
        point: 0x2F9D8,
        offset: 3057,
        len: 1,
    },
    Decomposition {
        point: 0x2F9D9,
        offset: 3058,
        len: 1,
    },
    Decomposition {
        point: 0x2F9DA,
        offset: 3059,
        len: 1,
    },
    Decomposition {
        point: 0x2F9DB,
        offset: 3060,
        len: 1,
    },
    Decomposition {
        point: 0x2F9DC,
        offset: 3061,
        len: 1,
    },
    Decomposition {
        point: 0x2F9DD,
        offset: 3062,
        len: 1,
    },
    Decomposition {
        point: 0x2F9DE,
        offset: 3063,
        len: 1,
    },
    Decomposition {
        point: 0x2F9DF,
        offset: 3064,
        len: 1,
    },
    Decomposition {
        point: 0x2F9E0,
        offset: 3065,
        len: 1,
    },
    Decomposition {
        point: 0x2F9E1,
        offset: 3066,
        len: 1,
    },
    Decomposition {
        point: 0x2F9E2,
        offset: 3067,
        len: 1,
    },
    Decomposition {
        point: 0x2F9E3,
        offset: 3068,
        len: 1,
    },
    Decomposition {
        point: 0x2F9E4,
        offset: 3069,
        len: 1,
    },
    Decomposition {
        point: 0x2F9E5,
        offset: 3070,
        len: 1,
    },
    Decomposition {
        point: 0x2F9E6,
        offset: 3071,
        len: 1,
    },
    Decomposition {
        point: 0x2F9E7,
        offset: 3072,
        len: 1,
    },
    Decomposition {
        point: 0x2F9E8,
        offset: 3073,
        len: 1,
    },
    Decomposition {
        point: 0x2F9E9,
        offset: 3074,
        len: 1,
    },
    Decomposition {
        point: 0x2F9EA,
        offset: 3075,
        len: 1,
    },
    Decomposition {
        point: 0x2F9EB,
        offset: 3076,
        len: 1,
    },
    Decomposition {
        point: 0x2F9EC,
        offset: 3077,
        len: 1,
    },
    Decomposition {
        point: 0x2F9ED,
        offset: 3078,
        len: 1,
    },
    Decomposition {
        point: 0x2F9EE,
        offset: 3079,
        len: 1,
    },
    Decomposition {
        point: 0x2F9EF,
        offset: 3080,
        len: 1,
    },
    Decomposition {
        point: 0x2F9F0,
        offset: 3081,
        len: 1,
    },
    Decomposition {
        point: 0x2F9F1,
        offset: 3082,
        len: 1,
    },
    Decomposition {
        point: 0x2F9F2,
        offset: 3083,
        len: 1,
    },
    Decomposition {
        point: 0x2F9F3,
        offset: 3084,
        len: 1,
    },
    Decomposition {
        point: 0x2F9F4,
        offset: 3085,
        len: 1,
    },
    Decomposition {
        point: 0x2F9F5,
        offset: 3086,
        len: 1,
    },
    Decomposition {
        point: 0x2F9F6,
        offset: 3087,
        len: 1,
    },
    Decomposition {
        point: 0x2F9F7,
        offset: 3088,
        len: 1,
    },
    Decomposition {
        point: 0x2F9F8,
        offset: 3089,
        len: 1,
    },
    Decomposition {
        point: 0x2F9F9,
        offset: 3090,
        len: 1,
    },
    Decomposition {
        point: 0x2F9FA,
        offset: 3091,
        len: 1,
    },
    Decomposition {
        point: 0x2F9FB,
        offset: 3092,
        len: 1,
    },
    Decomposition {
        point: 0x2F9FC,
        offset: 3093,
        len: 1,
    },
    Decomposition {
        point: 0x2F9FD,
        offset: 3094,
        len: 1,
    },
    Decomposition {
        point: 0x2F9FE,
        offset: 3095,
        len: 1,
    },
    Decomposition {
        point: 0x2F9FF,
        offset: 3096,
        len: 1,
    },
    Decomposition {
        point: 0x2FA00,
        offset: 3097,
        len: 1,
    },
    Decomposition {
        point: 0x2FA01,
        offset: 3098,
        len: 1,
    },
    Decomposition {
        point: 0x2FA02,
        offset: 3099,
        len: 1,
    },
    Decomposition {
        point: 0x2FA03,
        offset: 3100,
        len: 1,
    },
    Decomposition {
        point: 0x2FA04,
        offset: 3101,
        len: 1,
    },
    Decomposition {
        point: 0x2FA05,
        offset: 3102,
        len: 1,
    },
    Decomposition {
        point: 0x2FA06,
        offset: 3103,
        len: 1,
    },
    Decomposition {
        point: 0x2FA07,
        offset: 3104,
        len: 1,
    },
    Decomposition {
        point: 0x2FA08,
        offset: 3105,
        len: 1,
    },
    Decomposition {
        point: 0x2FA09,
        offset: 3106,
        len: 1,
    },
    Decomposition {
        point: 0x2FA0A,
        offset: 3107,
        len: 1,
    },
    Decomposition {
        point: 0x2FA0B,
        offset: 3108,
        len: 1,
    },
    Decomposition {
        point: 0x2FA0C,
        offset: 3109,
        len: 1,
    },
    Decomposition {
        point: 0x2FA0D,
        offset: 3110,
        len: 1,
    },
    Decomposition {
        point: 0x2FA0E,
        offset: 3111,
        len: 1,
    },
    Decomposition {
        point: 0x2FA0F,
        offset: 3112,
        len: 1,
    },
    Decomposition {
        point: 0x2FA10,
        offset: 3113,
        len: 1,
    },
    Decomposition {
        point: 0x2FA11,
        offset: 3114,
        len: 1,
    },
    Decomposition {
        point: 0x2FA12,
        offset: 3115,
        len: 1,
    },
    Decomposition {
        point: 0x2FA13,
        offset: 3116,
        len: 1,
    },
    Decomposition {
        point: 0x2FA14,
        offset: 3117,
        len: 1,
    },
    Decomposition {
        point: 0x2FA15,
        offset: 3118,
        len: 1,
    },
    Decomposition {
        point: 0x2FA16,
        offset: 3119,
        len: 1,
    },
    Decomposition {
        point: 0x2FA17,
        offset: 3120,
        len: 1,
    },
    Decomposition {
        point: 0x2FA18,
        offset: 3121,
        len: 1,
    },
    Decomposition {
        point: 0x2FA19,
        offset: 3122,
        len: 1,
    },
    Decomposition {
        point: 0x2FA1A,
        offset: 3123,
        len: 1,
    },
    Decomposition {
        point: 0x2FA1B,
        offset: 3124,
        len: 1,
    },
    Decomposition {
        point: 0x2FA1C,
        offset: 3125,
        len: 1,
    },
    Decomposition {
        point: 0x2FA1D,
        offset: 3126,
        len: 1,
    },
];

pub const DECOMPOSITION_POINTS: &[u32] = &[
    0x41, 0x300, 0x41, 0x301, 0x41, 0x302, 0x41, 0x303, 0x41, 0x308, 0x41, 0x30A, 0x43, 0x327,
    0x45, 0x300, 0x45, 0x301, 0x45, 0x302, 0x45, 0x308, 0x49, 0x300, 0x49, 0x301, 0x49, 0x302,
    0x49, 0x308, 0x4E, 0x303, 0x4F, 0x300, 0x4F, 0x301, 0x4F, 0x302, 0x4F, 0x303, 0x4F, 0x308,
    0x55, 0x300, 0x55, 0x301, 0x55, 0x302, 0x55, 0x308, 0x59, 0x301, 0x61, 0x300, 0x61, 0x301,
    0x61, 0x302, 0x61, 0x303, 0x61, 0x308, 0x61, 0x30A, 0x63, 0x327, 0x65, 0x300, 0x65, 0x301,
    0x65, 0x302, 0x65, 0x308, 0x69, 0x300, 0x69, 0x301, 0x69, 0x302, 0x69, 0x308, 0x6E, 0x303,
    0x6F, 0x300, 0x6F, 0x301, 0x6F, 0x302, 0x6F, 0x303, 0x6F, 0x308, 0x75, 0x300, 0x75, 0x301,
    0x75, 0x302, 0x75, 0x308, 0x79, 0x301, 0x79, 0x308, 0x41, 0x304, 0x61, 0x304, 0x41, 0x306,
    0x61, 0x306, 0x41, 0x328, 0x61, 0x328, 0x43, 0x301, 0x63, 0x301, 0x43, 0x302, 0x63, 0x302,
    0x43, 0x307, 0x63, 0x307, 0x43, 0x30C, 0x63, 0x30C, 0x44, 0x30C, 0x64, 0x30C, 0x45, 0x304,
    0x65, 0x304, 0x45, 0x306, 0x65, 0x306, 0x45, 0x307, 0x65, 0x307, 0x45, 0x328, 0x65, 0x328,
    0x45, 0x30C, 0x65, 0x30C, 0x47, 0x302, 0x67, 0x302, 0x47, 0x306, 0x67, 0x306, 0x47, 0x307,
    0x67, 0x307, 0x47, 0x327, 0x67, 0x327, 0x48, 0x302, 0x68, 0x302, 0x49, 0x303, 0x69, 0x303,
    0x49, 0x304, 0x69, 0x304, 0x49, 0x306, 0x69, 0x306, 0x49, 0x328, 0x69, 0x328, 0x49, 0x307,
    0x4A, 0x302, 0x6A, 0x302, 0x4B, 0x327, 0x6B, 0x327, 0x4C, 0x301, 0x6C, 0x301, 0x4C, 0x327,
    0x6C, 0x327, 0x4C, 0x30C, 0x6C, 0x30C, 0x4E, 0x301, 0x6E, 0x301, 0x4E, 0x327, 0x6E, 0x327,
    0x4E, 0x30C, 0x6E, 0x30C, 0x4F, 0x304, 0x6F, 0x304, 0x4F, 0x306, 0x6F, 0x306, 0x4F, 0x30B,
    0x6F, 0x30B, 0x52, 0x301, 0x72, 0x301, 0x52, 0x327, 0x72, 0x327, 0x52, 0x30C, 0x72, 0x30C,
    0x53, 0x301, 0x73, 0x301, 0x53, 0x302, 0x73, 0x302, 0x53, 0x327, 0x73, 0x327, 0x53, 0x30C,
    0x73, 0x30C, 0x54, 0x327, 0x74, 0x327, 0x54, 0x30C, 0x74, 0x30C, 0x55, 0x303, 0x75, 0x303,
    0x55, 0x304, 0x75, 0x304, 0x55, 0x306, 0x75, 0x306, 0x55, 0x30A, 0x75, 0x30A, 0x55, 0x30B,
    0x75, 0x30B, 0x55, 0x328, 0x75, 0x328, 0x57, 0x302, 0x77, 0x302, 0x59, 0x302, 0x79, 0x302,
    0x59, 0x308, 0x5A, 0x301, 0x7A, 0x301, 0x5A, 0x307, 0x7A, 0x307, 0x5A, 0x30C, 0x7A, 0x30C,
    0x4F, 0x31B, 0x6F, 0x31B, 0x55, 0x31B, 0x75, 0x31B, 0x41, 0x30C, 0x61, 0x30C, 0x49, 0x30C,
    0x69, 0x30C, 0x4F, 0x30C, 0x6F, 0x30C, 0x55, 0x30C, 0x75, 0x30C, 0xDC, 0x304, 0xFC, 0x304,
    0xDC, 0x301, 0xFC, 0x301, 0xDC, 0x30C, 0xFC, 0x30C, 0xDC, 0x300, 0xFC, 0x300, 0xC4, 0x304,
    0xE4, 0x304, 0x226, 0x304, 0x227, 0x304, 0xC6, 0x304, 0xE6, 0x304, 0x47, 0x30C, 0x67, 0x30C,
    0x4B, 0x30C, 0x6B, 0x30C, 0x4F, 0x328, 0x6F, 0x328, 0x1EA, 0x304, 0x1EB, 0x304, 0x1B7, 0x30C,
    0x292, 0x30C, 0x6A, 0x30C, 0x47, 0x301, 0x67, 0x301, 0x4E, 0x300, 0x6E, 0x300, 0xC5, 0x301,
    0xE5, 0x301, 0xC6, 0x301, 0xE6, 0x301, 0xD8, 0x301, 0xF8, 0x301, 0x41, 0x30F, 0x61, 0x30F,
    0x41, 0x311, 0x61, 0x311, 0x45, 0x30F, 0x65, 0x30F, 0x45, 0x311, 0x65, 0x311, 0x49, 0x30F,
    0x69, 0x30F, 0x49, 0x311, 0x69, 0x311, 0x4F, 0x30F, 0x6F, 0x30F, 0x4F, 0x311, 0x6F, 0x311,
    0x52, 0x30F, 0x72, 0x30F, 0x52, 0x311, 0x72, 0x311, 0x55, 0x30F, 0x75, 0x30F, 0x55, 0x311,
    0x75, 0x311, 0x53, 0x326, 0x73, 0x326, 0x54, 0x326, 0x74, 0x326, 0x48, 0x30C, 0x68, 0x30C,
    0x41, 0x307, 0x61, 0x307, 0x45, 0x327, 0x65, 0x327, 0xD6, 0x304, 0xF6, 0x304, 0xD5, 0x304,
    0xF5, 0x304, 0x4F, 0x307, 0x6F, 0x307, 0x22E, 0x304, 0x22F, 0x304, 0x59, 0x304, 0x79, 0x304,
    0x300, 0x301, 0x313, 0x308, 0x301, 0x2B9, 0x3B, 0xA8, 0x301, 0x391, 0x301, 0xB7, 0x395, 0x301,
    0x397, 0x301, 0x399, 0x301, 0x39F, 0x301, 0x3A5, 0x301, 0x3A9, 0x301, 0x3CA, 0x301, 0x399,
    0x308, 0x3A5, 0x308, 0x3B1, 0x301, 0x3B5, 0x301, 0x3B7, 0x301, 0x3B9, 0x301, 0x3CB, 0x301,
    0x3B9, 0x308, 0x3C5, 0x308, 0x3BF, 0x301, 0x3C5, 0x301, 0x3C9, 0x301, 0x3D2, 0x301, 0x3D2,
    0x308, 0x415, 0x300, 0x415, 0x308, 0x413, 0x301, 0x406, 0x308, 0x41A, 0x301, 0x418, 0x300,
    0x423, 0x306, 0x418, 0x306, 0x438, 0x306, 0x435, 0x300, 0x435, 0x308, 0x433, 0x301, 0x456,
    0x308, 0x43A, 0x301, 0x438, 0x300, 0x443, 0x306, 0x474, 0x30F, 0x475, 0x30F, 0x416, 0x306,
    0x436, 0x306, 0x410, 0x306, 0x430, 0x306, 0x410, 0x308, 0x430, 0x308, 0x415, 0x306, 0x435,
    0x306, 0x4D8, 0x308, 0x4D9, 0x308, 0x416, 0x308, 0x436, 0x308, 0x417, 0x308, 0x437, 0x308,
    0x418, 0x304, 0x438, 0x304, 0x418, 0x308, 0x438, 0x308, 0x41E, 0x308, 0x43E, 0x308, 0x4E8,
    0x308, 0x4E9, 0x308, 0x42D, 0x308, 0x44D, 0x308, 0x423, 0x304, 0x443, 0x304, 0x423, 0x308,
    0x443, 0x308, 0x423, 0x30B, 0x443, 0x30B, 0x427, 0x308, 0x447, 0x308, 0x42B, 0x308, 0x44B,
    0x308, 0x627, 0x653, 0x627, 0x654, 0x648, 0x654, 0x627, 0x655, 0x64A, 0x654, 0x6D5, 0x654,
    0x6C1, 0x654, 0x6D2, 0x654, 0x928, 0x93C, 0x930, 0x93C, 0x933, 0x93C, 0x915, 0x93C, 0x916,
    0x93C, 0x917, 0x93C, 0x91C, 0x93C, 0x921, 0x93C, 0x922, 0x93C, 0x92B, 0x93C, 0x92F, 0x93C,
    0x9C7, 0x9BE, 0x9C7, 0x9D7, 0x9A1, 0x9BC, 0x9A2, 0x9BC, 0x9AF, 0x9BC, 0xA32, 0xA3C, 0xA38,
    0xA3C, 0xA16, 0xA3C, 0xA17, 0xA3C, 0xA1C, 0xA3C, 0xA2B, 0xA3C, 0xB47, 0xB56, 0xB47, 0xB3E,
    0xB47, 0xB57, 0xB21, 0xB3C, 0xB22, 0xB3C, 0xB92, 0xBD7, 0xBC6, 0xBBE, 0xBC7, 0xBBE, 0xBC6,
    0xBD7, 0xC46, 0xC56, 0xCBF, 0xCD5, 0xCC6, 0xCD5, 0xCC6, 0xCD6, 0xCC6, 0xCC2, 0xCCA, 0xCD5,
    0xD46, 0xD3E, 0xD47, 0xD3E, 0xD46, 0xD57, 0xDD9, 0xDCA, 0xDD9, 0xDCF, 0xDDC, 0xDCA, 0xDD9,
    0xDDF, 0xF42, 0xFB7, 0xF4C, 0xFB7, 0xF51, 0xFB7, 0xF56, 0xFB7, 0xF5B, 0xFB7, 0xF40, 0xFB5,
    0xF71, 0xF72, 0xF71, 0xF74, 0xFB2, 0xF80, 0xFB3, 0xF80, 0xF71, 0xF80, 0xF92, 0xFB7, 0xF9C,
    0xFB7, 0xFA1, 0xFB7, 0xFA6, 0xFB7, 0xFAB, 0xFB7, 0xF90, 0xFB5, 0x1025, 0x102E, 0x1B05, 0x1B35,
    0x1B07, 0x1B35, 0x1B09, 0x1B35, 0x1B0B, 0x1B35, 0x1B0D, 0x1B35, 0x1B11, 0x1B35, 0x1B3A, 0x1B35,
    0x1B3C, 0x1B35, 0x1B3E, 0x1B35, 0x1B3F, 0x1B35, 0x1B42, 0x1B35, 0x41, 0x325, 0x61, 0x325, 0x42,
    0x307, 0x62, 0x307, 0x42, 0x323, 0x62, 0x323, 0x42, 0x331, 0x62, 0x331, 0xC7, 0x301, 0xE7,
    0x301, 0x44, 0x307, 0x64, 0x307, 0x44, 0x323, 0x64, 0x323, 0x44, 0x331, 0x64, 0x331, 0x44,
    0x327, 0x64, 0x327, 0x44, 0x32D, 0x64, 0x32D, 0x112, 0x300, 0x113, 0x300, 0x112, 0x301, 0x113,
    0x301, 0x45, 0x32D, 0x65, 0x32D, 0x45, 0x330, 0x65, 0x330, 0x228, 0x306, 0x229, 0x306, 0x46,
    0x307, 0x66, 0x307, 0x47, 0x304, 0x67, 0x304, 0x48, 0x307, 0x68, 0x307, 0x48, 0x323, 0x68,
    0x323, 0x48, 0x308, 0x68, 0x308, 0x48, 0x327, 0x68, 0x327, 0x48, 0x32E, 0x68, 0x32E, 0x49,
    0x330, 0x69, 0x330, 0xCF, 0x301, 0xEF, 0x301, 0x4B, 0x301, 0x6B, 0x301, 0x4B, 0x323, 0x6B,
    0x323, 0x4B, 0x331, 0x6B, 0x331, 0x4C, 0x323, 0x6C, 0x323, 0x1E36, 0x304, 0x1E37, 0x304, 0x4C,
    0x331, 0x6C, 0x331, 0x4C, 0x32D, 0x6C, 0x32D, 0x4D, 0x301, 0x6D, 0x301, 0x4D, 0x307, 0x6D,
    0x307, 0x4D, 0x323, 0x6D, 0x323, 0x4E, 0x307, 0x6E, 0x307, 0x4E, 0x323, 0x6E, 0x323, 0x4E,
    0x331, 0x6E, 0x331, 0x4E, 0x32D, 0x6E, 0x32D, 0xD5, 0x301, 0xF5, 0x301, 0xD5, 0x308, 0xF5,
    0x308, 0x14C, 0x300, 0x14D, 0x300, 0x14C, 0x301, 0x14D, 0x301, 0x50, 0x301, 0x70, 0x301, 0x50,
    0x307, 0x70, 0x307, 0x52, 0x307, 0x72, 0x307, 0x52, 0x323, 0x72, 0x323, 0x1E5A, 0x304, 0x1E5B,
    0x304, 0x52, 0x331, 0x72, 0x331, 0x53, 0x307, 0x73, 0x307, 0x53, 0x323, 0x73, 0x323, 0x15A,
    0x307, 0x15B, 0x307, 0x160, 0x307, 0x161, 0x307, 0x1E62, 0x307, 0x1E63, 0x307, 0x54, 0x307,
    0x74, 0x307, 0x54, 0x323, 0x74, 0x323, 0x54, 0x331, 0x74, 0x331, 0x54, 0x32D, 0x74, 0x32D,
    0x55, 0x324, 0x75, 0x324, 0x55, 0x330, 0x75, 0x330, 0x55, 0x32D, 0x75, 0x32D, 0x168, 0x301,
    0x169, 0x301, 0x16A, 0x308, 0x16B, 0x308, 0x56, 0x303, 0x76, 0x303, 0x56, 0x323, 0x76, 0x323,
    0x57, 0x300, 0x77, 0x300, 0x57, 0x301, 0x77, 0x301, 0x57, 0x308, 0x77, 0x308, 0x57, 0x307,
    0x77, 0x307, 0x57, 0x323, 0x77, 0x323, 0x58, 0x307, 0x78, 0x307, 0x58, 0x308, 0x78, 0x308,
    0x59, 0x307, 0x79, 0x307, 0x5A, 0x302, 0x7A, 0x302, 0x5A, 0x323, 0x7A, 0x323, 0x5A, 0x331,
    0x7A, 0x331, 0x68, 0x331, 0x74, 0x308, 0x77, 0x30A, 0x79, 0x30A, 0x17F, 0x307, 0x41, 0x323,
    0x61, 0x323, 0x41, 0x309, 0x61, 0x309, 0xC2, 0x301, 0xE2, 0x301, 0xC2, 0x300, 0xE2, 0x300,
    0xC2, 0x309, 0xE2, 0x309, 0xC2, 0x303, 0xE2, 0x303, 0x1EA0, 0x302, 0x1EA1, 0x302, 0x102, 0x301,
    0x103, 0x301, 0x102, 0x300, 0x103, 0x300, 0x102, 0x309, 0x103, 0x309, 0x102, 0x303, 0x103,
    0x303, 0x1EA0, 0x306, 0x1EA1, 0x306, 0x45, 0x323, 0x65, 0x323, 0x45, 0x309, 0x65, 0x309, 0x45,
    0x303, 0x65, 0x303, 0xCA, 0x301, 0xEA, 0x301, 0xCA, 0x300, 0xEA, 0x300, 0xCA, 0x309, 0xEA,
    0x309, 0xCA, 0x303, 0xEA, 0x303, 0x1EB8, 0x302, 0x1EB9, 0x302, 0x49, 0x309, 0x69, 0x309, 0x49,
    0x323, 0x69, 0x323, 0x4F, 0x323, 0x6F, 0x323, 0x4F, 0x309, 0x6F, 0x309, 0xD4, 0x301, 0xF4,
    0x301, 0xD4, 0x300, 0xF4, 0x300, 0xD4, 0x309, 0xF4, 0x309, 0xD4, 0x303, 0xF4, 0x303, 0x1ECC,
    0x302, 0x1ECD, 0x302, 0x1A0, 0x301, 0x1A1, 0x301, 0x1A0, 0x300, 0x1A1, 0x300, 0x1A0, 0x309,
    0x1A1, 0x309, 0x1A0, 0x303, 0x1A1, 0x303, 0x1A0, 0x323, 0x1A1, 0x323, 0x55, 0x323, 0x75, 0x323,
    0x55, 0x309, 0x75, 0x309, 0x1AF, 0x301, 0x1B0, 0x301, 0x1AF, 0x300, 0x1B0, 0x300, 0x1AF, 0x309,
    0x1B0, 0x309, 0x1AF, 0x303, 0x1B0, 0x303, 0x1AF, 0x323, 0x1B0, 0x323, 0x59, 0x300, 0x79, 0x300,
    0x59, 0x323, 0x79, 0x323, 0x59, 0x309, 0x79, 0x309, 0x59, 0x303, 0x79, 0x303, 0x3B1, 0x313,
    0x3B1, 0x314, 0x1F00, 0x300, 0x1F01, 0x300, 0x1F00, 0x301, 0x1F01, 0x301, 0x1F00, 0x342,
    0x1F01, 0x342, 0x391, 0x313, 0x391, 0x314, 0x1F08, 0x300, 0x1F09, 0x300, 0x1F08, 0x301, 0x1F09,
    0x301, 0x1F08, 0x342, 0x1F09, 0x342, 0x3B5, 0x313, 0x3B5, 0x314, 0x1F10, 0x300, 0x1F11, 0x300,
    0x1F10, 0x301, 0x1F11, 0x301, 0x395, 0x313, 0x395, 0x314, 0x1F18, 0x300, 0x1F19, 0x300, 0x1F18,
    0x301, 0x1F19, 0x301, 0x3B7, 0x313, 0x3B7, 0x314, 0x1F20, 0x300, 0x1F21, 0x300, 0x1F20, 0x301,
    0x1F21, 0x301, 0x1F20, 0x342, 0x1F21, 0x342, 0x397, 0x313, 0x397, 0x314, 0x1F28, 0x300, 0x1F29,
    0x300, 0x1F28, 0x301, 0x1F29, 0x301, 0x1F28, 0x342, 0x1F29, 0x342, 0x3B9, 0x313, 0x3B9, 0x314,
    0x1F30, 0x300, 0x1F31, 0x300, 0x1F30, 0x301, 0x1F31, 0x301, 0x1F30, 0x342, 0x1F31, 0x342,
    0x399, 0x313, 0x399, 0x314, 0x1F38, 0x300, 0x1F39, 0x300, 0x1F38, 0x301, 0x1F39, 0x301, 0x1F38,
    0x342, 0x1F39, 0x342, 0x3BF, 0x313, 0x3BF, 0x314, 0x1F40, 0x300, 0x1F41, 0x300, 0x1F40, 0x301,
    0x1F41, 0x301, 0x39F, 0x313, 0x39F, 0x314, 0x1F48, 0x300, 0x1F49, 0x300, 0x1F48, 0x301, 0x1F49,
    0x301, 0x3C5, 0x313, 0x3C5, 0x314, 0x1F50, 0x300, 0x1F51, 0x300, 0x1F50, 0x301, 0x1F51, 0x301,
    0x1F50, 0x342, 0x1F51, 0x342, 0x3A5, 0x314, 0x1F59, 0x300, 0x1F59, 0x301, 0x1F59, 0x342, 0x3C9,
    0x313, 0x3C9, 0x314, 0x1F60, 0x300, 0x1F61, 0x300, 0x1F60, 0x301, 0x1F61, 0x301, 0x1F60, 0x342,
    0x1F61, 0x342, 0x3A9, 0x313, 0x3A9, 0x314, 0x1F68, 0x300, 0x1F69, 0x300, 0x1F68, 0x301, 0x1F69,
    0x301, 0x1F68, 0x342, 0x1F69, 0x342, 0x3B1, 0x300, 0x3AC, 0x3B5, 0x300, 0x3AD, 0x3B7, 0x300,
    0x3AE, 0x3B9, 0x300, 0x3AF, 0x3BF, 0x300, 0x3CC, 0x3C5, 0x300, 0x3CD, 0x3C9, 0x300, 0x3CE,
    0x1F00, 0x345, 0x1F01, 0x345, 0x1F02, 0x345, 0x1F03, 0x345, 0x1F04, 0x345, 0x1F05, 0x345,
    0x1F06, 0x345, 0x1F07, 0x345, 0x1F08, 0x345, 0x1F09, 0x345, 0x1F0A, 0x345, 0x1F0B, 0x345,
    0x1F0C, 0x345, 0x1F0D, 0x345, 0x1F0E, 0x345, 0x1F0F, 0x345, 0x1F20, 0x345, 0x1F21, 0x345,
    0x1F22, 0x345, 0x1F23, 0x345, 0x1F24, 0x345, 0x1F25, 0x345, 0x1F26, 0x345, 0x1F27, 0x345,
    0x1F28, 0x345, 0x1F29, 0x345, 0x1F2A, 0x345, 0x1F2B, 0x345, 0x1F2C, 0x345, 0x1F2D, 0x345,
    0x1F2E, 0x345, 0x1F2F, 0x345, 0x1F60, 0x345, 0x1F61, 0x345, 0x1F62, 0x345, 0x1F63, 0x345,
    0x1F64, 0x345, 0x1F65, 0x345, 0x1F66, 0x345, 0x1F67, 0x345, 0x1F68, 0x345, 0x1F69, 0x345,
    0x1F6A, 0x345, 0x1F6B, 0x345, 0x1F6C, 0x345, 0x1F6D, 0x345, 0x1F6E, 0x345, 0x1F6F, 0x345,
    0x3B1, 0x306, 0x3B1, 0x304, 0x1F70, 0x345, 0x3B1, 0x345, 0x3AC, 0x345, 0x3B1, 0x342, 0x1FB6,
    0x345, 0x391, 0x306, 0x391, 0x304, 0x391, 0x300, 0x386, 0x391, 0x345, 0x3B9, 0xA8, 0x342,
    0x1F74, 0x345, 0x3B7, 0x345, 0x3AE, 0x345, 0x3B7, 0x342, 0x1FC6, 0x345, 0x395, 0x300, 0x388,
    0x397, 0x300, 0x389, 0x397, 0x345, 0x1FBF, 0x300, 0x1FBF, 0x301, 0x1FBF, 0x342, 0x3B9, 0x306,
    0x3B9, 0x304, 0x3CA, 0x300, 0x390, 0x3B9, 0x342, 0x3CA, 0x342, 0x399, 0x306, 0x399, 0x304,
    0x399, 0x300, 0x38A, 0x1FFE, 0x300, 0x1FFE, 0x301, 0x1FFE, 0x342, 0x3C5, 0x306, 0x3C5, 0x304,
    0x3CB, 0x300, 0x3B0, 0x3C1, 0x313, 0x3C1, 0x314, 0x3C5, 0x342, 0x3CB, 0x342, 0x3A5, 0x306,
    0x3A5, 0x304, 0x3A5, 0x300, 0x38E, 0x3A1, 0x314, 0xA8, 0x300, 0x385, 0x60, 0x1F7C, 0x345,
    0x3C9, 0x345, 0x3CE, 0x345, 0x3C9, 0x342, 0x1FF6, 0x345, 0x39F, 0x300, 0x38C, 0x3A9, 0x300,
    0x38F, 0x3A9, 0x345, 0xB4, 0x2002, 0x2003, 0x3A9, 0x4B, 0xC5, 0x2190, 0x338, 0x2192, 0x338,
    0x2194, 0x338, 0x21D0, 0x338, 0x21D4, 0x338, 0x21D2, 0x338, 0x2203, 0x338, 0x2208, 0x338,
    0x220B, 0x338, 0x2223, 0x338, 0x2225, 0x338, 0x223C, 0x338, 0x2243, 0x338, 0x2245, 0x338,
    0x2248, 0x338, 0x3D, 0x338, 0x2261, 0x338, 0x224D, 0x338, 0x3C, 0x338, 0x3E, 0x338, 0x2264,
    0x338, 0x2265, 0x338, 0x2272, 0x338, 0x2273, 0x338, 0x2276, 0x338, 0x2277, 0x338, 0x227A,
    0x338, 0x227B, 0x338, 0x2282, 0x338, 0x2283, 0x338, 0x2286, 0x338, 0x2287, 0x338, 0x22A2,
    0x338, 0x22A8, 0x338, 0x22A9, 0x338, 0x22AB, 0x338, 0x227C, 0x338, 0x227D, 0x338, 0x2291,
    0x338, 0x2292, 0x338, 0x22B2, 0x338, 0x22B3, 0x338, 0x22B4, 0x338, 0x22B5, 0x338, 0x3008,
    0x3009, 0x2ADD, 0x338, 0x304B, 0x3099, 0x304D, 0x3099, 0x304F, 0x3099, 0x3051, 0x3099, 0x3053,
    0x3099, 0x3055, 0x3099, 0x3057, 0x3099, 0x3059, 0x3099, 0x305B, 0x3099, 0x305D, 0x3099, 0x305F,
    0x3099, 0x3061, 0x3099, 0x3064, 0x3099, 0x3066, 0x3099, 0x3068, 0x3099, 0x306F, 0x3099, 0x306F,
    0x309A, 0x3072, 0x3099, 0x3072, 0x309A, 0x3075, 0x3099, 0x3075, 0x309A, 0x3078, 0x3099, 0x3078,
    0x309A, 0x307B, 0x3099, 0x307B, 0x309A, 0x3046, 0x3099, 0x309D, 0x3099, 0x30AB, 0x3099, 0x30AD,
    0x3099, 0x30AF, 0x3099, 0x30B1, 0x3099, 0x30B3, 0x3099, 0x30B5, 0x3099, 0x30B7, 0x3099, 0x30B9,
    0x3099, 0x30BB, 0x3099, 0x30BD, 0x3099, 0x30BF, 0x3099, 0x30C1, 0x3099, 0x30C4, 0x3099, 0x30C6,
    0x3099, 0x30C8, 0x3099, 0x30CF, 0x3099, 0x30CF, 0x309A, 0x30D2, 0x3099, 0x30D2, 0x309A, 0x30D5,
    0x3099, 0x30D5, 0x309A, 0x30D8, 0x3099, 0x30D8, 0x309A, 0x30DB, 0x3099, 0x30DB, 0x309A, 0x30A6,
    0x3099, 0x30EF, 0x3099, 0x30F0, 0x3099, 0x30F1, 0x3099, 0x30F2, 0x3099, 0x30FD, 0x3099, 0x8C48,
    0x66F4, 0x8ECA, 0x8CC8, 0x6ED1, 0x4E32, 0x53E5, 0x9F9C, 0x9F9C, 0x5951, 0x91D1, 0x5587, 0x5948,
    0x61F6, 0x7669, 0x7F85, 0x863F, 0x87BA, 0x88F8, 0x908F, 0x6A02, 0x6D1B, 0x70D9, 0x73DE, 0x843D,
    0x916A, 0x99F1, 0x4E82, 0x5375, 0x6B04, 0x721B, 0x862D, 0x9E1E, 0x5D50, 0x6FEB, 0x85CD, 0x8964,
    0x62C9, 0x81D8, 0x881F, 0x5ECA, 0x6717, 0x6D6A, 0x72FC, 0x90CE, 0x4F86, 0x51B7, 0x52DE, 0x64C4,
    0x6AD3, 0x7210, 0x76E7, 0x8001, 0x8606, 0x865C, 0x8DEF, 0x9732, 0x9B6F, 0x9DFA, 0x788C, 0x797F,
    0x7DA0, 0x83C9, 0x9304, 0x9E7F, 0x8AD6, 0x58DF, 0x5F04, 0x7C60, 0x807E, 0x7262, 0x78CA, 0x8CC2,
    0x96F7, 0x58D8, 0x5C62, 0x6A13, 0x6DDA, 0x6F0F, 0x7D2F, 0x7E37, 0x964B, 0x52D2, 0x808B, 0x51DC,
    0x51CC, 0x7A1C, 0x7DBE, 0x83F1, 0x9675, 0x8B80, 0x62CF, 0x6A02, 0x8AFE, 0x4E39, 0x5BE7, 0x6012,
    0x7387, 0x7570, 0x5317, 0x78FB, 0x4FBF, 0x5FA9, 0x4E0D, 0x6CCC, 0x6578, 0x7D22, 0x53C3, 0x585E,
    0x7701, 0x8449, 0x8AAA, 0x6BBA, 0x8FB0, 0x6C88, 0x62FE, 0x82E5, 0x63A0, 0x7565, 0x4EAE, 0x5169,
    0x51C9, 0x6881, 0x7CE7, 0x826F, 0x8AD2, 0x91CF, 0x52F5, 0x5442, 0x5973, 0x5EEC, 0x65C5, 0x6FFE,
    0x792A, 0x95AD, 0x9A6A, 0x9E97, 0x9ECE, 0x529B, 0x66C6, 0x6B77, 0x8F62, 0x5E74, 0x6190, 0x6200,
    0x649A, 0x6F23, 0x7149, 0x7489, 0x79CA, 0x7DF4, 0x806F, 0x8F26, 0x84EE, 0x9023, 0x934A, 0x5217,
    0x52A3, 0x54BD, 0x70C8, 0x88C2, 0x8AAA, 0x5EC9, 0x5FF5, 0x637B, 0x6BAE, 0x7C3E, 0x7375, 0x4EE4,
    0x56F9, 0x5BE7, 0x5DBA, 0x601C, 0x73B2, 0x7469, 0x7F9A, 0x8046, 0x9234, 0x96F6, 0x9748, 0x9818,
    0x4F8B, 0x79AE, 0x91B4, 0x96B8, 0x60E1, 0x4E86, 0x50DA, 0x5BEE, 0x5C3F, 0x6599, 0x6A02, 0x71CE,
    0x7642, 0x84FC, 0x907C, 0x9F8D, 0x6688, 0x962E, 0x5289, 0x677B, 0x67F3, 0x6D41, 0x6E9C, 0x7409,
    0x7559, 0x786B, 0x7D10, 0x985E, 0x516D, 0x622E, 0x9678, 0x502B, 0x5D19, 0x6DEA, 0x8F2A, 0x5F8B,
    0x6144, 0x6817, 0x7387, 0x9686, 0x5229, 0x540F, 0x5C65, 0x6613, 0x674E, 0x68A8, 0x6CE5, 0x7406,
    0x75E2, 0x7F79, 0x88CF, 0x88E1, 0x91CC, 0x96E2, 0x533F, 0x6EBA, 0x541D, 0x71D0, 0x7498, 0x85FA,
    0x96A3, 0x9C57, 0x9E9F, 0x6797, 0x6DCB, 0x81E8, 0x7ACB, 0x7B20, 0x7C92, 0x72C0, 0x7099, 0x8B58,
    0x4EC0, 0x8336, 0x523A, 0x5207, 0x5EA6, 0x62D3, 0x7CD6, 0x5B85, 0x6D1E, 0x66B4, 0x8F3B, 0x884C,
    0x964D, 0x898B, 0x5ED3, 0x5140, 0x55C0, 0x585A, 0x6674, 0x51DE, 0x732A, 0x76CA, 0x793C, 0x795E,
    0x7965, 0x798F, 0x9756, 0x7CBE, 0x7FBD, 0x8612, 0x8AF8, 0x9038, 0x90FD, 0x98EF, 0x98FC, 0x9928,
    0x9DB4, 0x90DE, 0x96B7, 0x4FAE, 0x50E7, 0x514D, 0x52C9, 0x52E4, 0x5351, 0x559D, 0x5606, 0x5668,
    0x5840, 0x58A8, 0x5C64, 0x5C6E, 0x6094, 0x6168, 0x618E, 0x61F2, 0x654F, 0x65E2, 0x6691, 0x6885,
    0x6D77, 0x6E1A, 0x6F22, 0x716E, 0x722B, 0x7422, 0x7891, 0x793E, 0x7949, 0x7948, 0x7950, 0x7956,
    0x795D, 0x798D, 0x798E, 0x7A40, 0x7A81, 0x7BC0, 0x7DF4, 0x7E09, 0x7E41, 0x7F72, 0x8005, 0x81ED,
    0x8279, 0x8279, 0x8457, 0x8910, 0x8996, 0x8B01, 0x8B39, 0x8CD3, 0x8D08, 0x8FB6, 0x9038, 0x96E3,
    0x97FF, 0x983B, 0x6075, 0x242EE, 0x8218, 0x4E26, 0x51B5, 0x5168, 0x4F80, 0x5145, 0x5180,
    0x52C7, 0x52FA, 0x559D, 0x5555, 0x5599, 0x55E2, 0x585A, 0x58B3, 0x5944, 0x5954, 0x5A62, 0x5B28,
    0x5ED2, 0x5ED9, 0x5F69, 0x5FAD, 0x60D8, 0x614E, 0x6108, 0x618E, 0x6160, 0x61F2, 0x6234, 0x63C4,
    0x641C, 0x6452, 0x6556, 0x6674, 0x6717, 0x671B, 0x6756, 0x6B79, 0x6BBA, 0x6D41, 0x6EDB, 0x6ECB,
    0x6F22, 0x701E, 0x716E, 0x77A7, 0x7235, 0x72AF, 0x732A, 0x7471, 0x7506, 0x753B, 0x761D, 0x761F,
    0x76CA, 0x76DB, 0x76F4, 0x774A, 0x7740, 0x78CC, 0x7AB1, 0x7BC0, 0x7C7B, 0x7D5B, 0x7DF4, 0x7F3E,
    0x8005, 0x8352, 0x83EF, 0x8779, 0x8941, 0x8986, 0x8996, 0x8ABF, 0x8AF8, 0x8ACB, 0x8B01, 0x8AFE,
    0x8AED, 0x8B39, 0x8B8A, 0x8D08, 0x8F38, 0x9072, 0x9199, 0x9276, 0x967C, 0x96E3, 0x9756, 0x97DB,
    0x97FF, 0x980B, 0x983B, 0x9B12, 0x9F9C, 0x2284A, 0x22844, 0x233D5, 0x3B9D, 0x4018, 0x4039,
    0x25249, 0x25CD0, 0x27ED3, 0x9F43, 0x9F8E, 0x5D9, 0x5B4, 0x5F2, 0x5B7, 0x5E9, 0x5C1, 0x5E9,
    0x5C2, 0xFB49, 0x5C1, 0xFB49, 0x5C2, 0x5D0, 0x5B7, 0x5D0, 0x5B8, 0x5D0, 0x5BC, 0x5D1, 0x5BC,
    0x5D2, 0x5BC, 0x5D3, 0x5BC, 0x5D4, 0x5BC, 0x5D5, 0x5BC, 0x5D6, 0x5BC, 0x5D8, 0x5BC, 0x5D9,
    0x5BC, 0x5DA, 0x5BC, 0x5DB, 0x5BC, 0x5DC, 0x5BC, 0x5DE, 0x5BC, 0x5E0, 0x5BC, 0x5E1, 0x5BC,
    0x5E3, 0x5BC, 0x5E4, 0x5BC, 0x5E6, 0x5BC, 0x5E7, 0x5BC, 0x5E8, 0x5BC, 0x5E9, 0x5BC, 0x5EA,
    0x5BC, 0x5D5, 0x5B9, 0x5D1, 0x5BF, 0x5DB, 0x5BF, 0x5E4, 0x5BF, 0x105D2, 0x307, 0x105DA, 0x307,
    0x11099, 0x110BA, 0x1109B, 0x110BA, 0x110A5, 0x110BA, 0x11131, 0x11127, 0x11132, 0x11127,
    0x11347, 0x1133E, 0x11347, 0x11357, 0x11382, 0x113C9, 0x11384, 0x113BB, 0x1138B, 0x113C2,
    0x11390, 0x113C9, 0x113C2, 0x113C2, 0x113C2, 0x113B8, 0x113C2, 0x113C9, 0x114B9, 0x114BA,
    0x114B9, 0x114B0, 0x114B9, 0x114BD, 0x115B8, 0x115AF, 0x115B9, 0x115AF, 0x11935, 0x11930,
    0x1611E, 0x1611E, 0x1611E, 0x16129, 0x1611E, 0x1611F, 0x16129, 0x1611F, 0x1611E, 0x16120,
    0x16121, 0x1611F, 0x16122, 0x1611F, 0x16121, 0x16120, 0x16D67, 0x16D67, 0x16D63, 0x16D67,
    0x16D69, 0x16D67, 0x1D157, 0x1D165, 0x1D158, 0x1D165, 0x1D15F, 0x1D16E, 0x1D15F, 0x1D16F,
    0x1D15F, 0x1D170, 0x1D15F, 0x1D171, 0x1D15F, 0x1D172, 0x1D1B9, 0x1D165, 0x1D1BA, 0x1D165,
    0x1D1BB, 0x1D16E, 0x1D1BC, 0x1D16E, 0x1D1BB, 0x1D16F, 0x1D1BC, 0x1D16F, 0x4E3D, 0x4E38, 0x4E41,
    0x20122, 0x4F60, 0x4FAE, 0x4FBB, 0x5002, 0x507A, 0x5099, 0x50E7, 0x50CF, 0x349E, 0x2063A,
    0x514D, 0x5154, 0x5164, 0x5177, 0x2051C, 0x34B9, 0x5167, 0x518D, 0x2054B, 0x5197, 0x51A4,
    0x4ECC, 0x51AC, 0x51B5, 0x291DF, 0x51F5, 0x5203, 0x34DF, 0x523B, 0x5246, 0x5272, 0x5277,
    0x3515, 0x52C7, 0x52C9, 0x52E4, 0x52FA, 0x5305, 0x5306, 0x5317, 0x5349, 0x5351, 0x535A, 0x5373,
    0x537D, 0x537F, 0x537F, 0x537F, 0x20A2C, 0x7070, 0x53CA, 0x53DF, 0x20B63, 0x53EB, 0x53F1,
    0x5406, 0x549E, 0x5438, 0x5448, 0x5468, 0x54A2, 0x54F6, 0x5510, 0x5553, 0x5563, 0x5584, 0x5584,
    0x5599, 0x55AB, 0x55B3, 0x55C2, 0x5716, 0x5606, 0x5717, 0x5651, 0x5674, 0x5207, 0x58EE, 0x57CE,
    0x57F4, 0x580D, 0x578B, 0x5832, 0x5831, 0x58AC, 0x214E4, 0x58F2, 0x58F7, 0x5906, 0x591A,
    0x5922, 0x5962, 0x216A8, 0x216EA, 0x59EC, 0x5A1B, 0x5A27, 0x59D8, 0x5A66, 0x36EE, 0x36FC,
    0x5B08, 0x5B3E, 0x5B3E, 0x219C8, 0x5BC3, 0x5BD8, 0x5BE7, 0x5BF3, 0x21B18, 0x5BFF, 0x5C06,
    0x5F53, 0x5C22, 0x3781, 0x5C60, 0x5C6E, 0x5CC0, 0x5C8D, 0x21DE4, 0x5D43, 0x21DE6, 0x5D6E,
    0x5D6B, 0x5D7C, 0x5DE1, 0x5DE2, 0x382F, 0x5DFD, 0x5E28, 0x5E3D, 0x5E69, 0x3862, 0x22183,
    0x387C, 0x5EB0, 0x5EB3, 0x5EB6, 0x5ECA, 0x2A392, 0x5EFE, 0x22331, 0x22331, 0x8201, 0x5F22,
    0x5F22, 0x38C7, 0x232B8, 0x261DA, 0x5F62, 0x5F6B, 0x38E3, 0x5F9A, 0x5FCD, 0x5FD7, 0x5FF9,
    0x6081, 0x393A, 0x391C, 0x6094, 0x226D4, 0x60C7, 0x6148, 0x614C, 0x614E, 0x614C, 0x617A,
    0x618E, 0x61B2, 0x61A4, 0x61AF, 0x61DE, 0x61F2, 0x61F6, 0x6210, 0x621B, 0x625D, 0x62B1, 0x62D4,
    0x6350, 0x22B0C, 0x633D, 0x62FC, 0x6368, 0x6383, 0x63E4, 0x22BF1, 0x6422, 0x63C5, 0x63A9,
    0x3A2E, 0x6469, 0x647E, 0x649D, 0x6477, 0x3A6C, 0x654F, 0x656C, 0x2300A, 0x65E3, 0x66F8,
    0x6649, 0x3B19, 0x6691, 0x3B08, 0x3AE4, 0x5192, 0x5195, 0x6700, 0x669C, 0x80AD, 0x43D9, 0x6717,
    0x671B, 0x6721, 0x675E, 0x6753, 0x233C3, 0x3B49, 0x67FA, 0x6785, 0x6852, 0x6885, 0x2346D,
    0x688E, 0x681F, 0x6914, 0x3B9D, 0x6942, 0x69A3, 0x69EA, 0x6AA8, 0x236A3, 0x6ADB, 0x3C18,
    0x6B21, 0x238A7, 0x6B54, 0x3C4E, 0x6B72, 0x6B9F, 0x6BBA, 0x6BBB, 0x23A8D, 0x21D0B, 0x23AFA,
    0x6C4E, 0x23CBC, 0x6CBF, 0x6CCD, 0x6C67, 0x6D16, 0x6D3E, 0x6D77, 0x6D41, 0x6D69, 0x6D78,
    0x6D85, 0x23D1E, 0x6D34, 0x6E2F, 0x6E6E, 0x3D33, 0x6ECB, 0x6EC7, 0x23ED1, 0x6DF9, 0x6F6E,
    0x23F5E, 0x23F8E, 0x6FC6, 0x7039, 0x701E, 0x701B, 0x3D96, 0x704A, 0x707D, 0x7077, 0x70AD,
    0x20525, 0x7145, 0x24263, 0x719C, 0x243AB, 0x7228, 0x7235, 0x7250, 0x24608, 0x7280, 0x7295,
    0x24735, 0x24814, 0x737A, 0x738B, 0x3EAC, 0x73A5, 0x3EB8, 0x3EB8, 0x7447, 0x745C, 0x7471,
    0x7485, 0x74CA, 0x3F1B, 0x7524, 0x24C36, 0x753E, 0x24C92, 0x7570, 0x2219F, 0x7610, 0x24FA1,
    0x24FB8, 0x25044, 0x3FFC, 0x4008, 0x76F4, 0x250F3, 0x250F2, 0x25119, 0x25133, 0x771E, 0x771F,
    0x771F, 0x774A, 0x4039, 0x778B, 0x4046, 0x4096, 0x2541D, 0x784E, 0x788C, 0x78CC, 0x40E3,
    0x25626, 0x7956, 0x2569A, 0x256C5, 0x798F, 0x79EB, 0x412F, 0x7A40, 0x7A4A, 0x7A4F, 0x2597C,
    0x25AA7, 0x25AA7, 0x7AEE, 0x4202, 0x25BAB, 0x7BC6, 0x7BC9, 0x4227, 0x25C80, 0x7CD2, 0x42A0,
    0x7CE8, 0x7CE3, 0x7D00, 0x25F86, 0x7D63, 0x4301, 0x7DC7, 0x7E02, 0x7E45, 0x4334, 0x26228,
    0x26247, 0x4359, 0x262D9, 0x7F7A, 0x2633E, 0x7F95, 0x7FFA, 0x8005, 0x264DA, 0x26523, 0x8060,
    0x265A8, 0x8070, 0x2335F, 0x43D5, 0x80B2, 0x8103, 0x440B, 0x813E, 0x5AB5, 0x267A7, 0x267B5,
    0x23393, 0x2339C, 0x8201, 0x8204, 0x8F9E, 0x446B, 0x8291, 0x828B, 0x829D, 0x52B3, 0x82B1,
    0x82B3, 0x82BD, 0x82E6, 0x26B3C, 0x82E5, 0x831D, 0x8363, 0x83AD, 0x8323, 0x83BD, 0x83E7,
    0x8457, 0x8353, 0x83CA, 0x83CC, 0x83DC, 0x26C36, 0x26D6B, 0x26CD5, 0x452B, 0x84F1, 0x84F3,
    0x8516, 0x273CA, 0x8564, 0x26F2C, 0x455D, 0x4561, 0x26FB1, 0x270D2, 0x456B, 0x8650, 0x865C,
    0x8667, 0x8669, 0x86A9, 0x8688, 0x870E, 0x86E2, 0x8779, 0x8728, 0x876B, 0x8786, 0x45D7, 0x87E1,
    0x8801, 0x45F9, 0x8860, 0x8863, 0x27667, 0x88D7, 0x88DE, 0x4635, 0x88FA, 0x34BB, 0x278AE,
    0x27966, 0x46BE, 0x46C7, 0x8AA0, 0x8AED, 0x8B8A, 0x8C55, 0x27CA8, 0x8CAB, 0x8CC1, 0x8D1B,
    0x8D77, 0x27F2F, 0x20804, 0x8DCB, 0x8DBC, 0x8DF0, 0x208DE, 0x8ED4, 0x8F38, 0x285D2, 0x285ED,
    0x9094, 0x90F1, 0x9111, 0x2872E, 0x911B, 0x9238, 0x92D7, 0x92D8, 0x927C, 0x93F9, 0x9415,
    0x28BFA, 0x958B, 0x4995, 0x95B7, 0x28D77, 0x49E6, 0x96C3, 0x5DB2, 0x9723, 0x29145, 0x2921A,
    0x4A6E, 0x4A76, 0x97E0, 0x2940A, 0x4AB2, 0x29496, 0x980B, 0x980B, 0x9829, 0x295B6, 0x98E2,
    0x4B33, 0x9929, 0x99A7, 0x99C2, 0x99FE, 0x4BCE, 0x29B30, 0x9B12, 0x9C40, 0x9CFD, 0x4CCE,
    0x4CED, 0x9D67, 0x2A0CE, 0x4CF8, 0x2A105, 0x2A20E, 0x2A291, 0x9EBB, 0x4D56, 0x9EF9, 0x9EFE,
    0x9F05, 0x9F0F, 0x9F16, 0x9F3B, 0x2A600,
];

pub const COMPOSITIONS: &[Composition] = &[
    Composition {
        first: 0x3C,
        second: 0x338,
        result: 0x226E,
    },
    Composition {
        first: 0x3D,
        second: 0x338,
        result: 0x2260,
    },
    Composition {
        first: 0x3E,
        second: 0x338,
        result: 0x226F,
    },
    Composition {
        first: 0x41,
        second: 0x300,
        result: 0xC0,
    },
    Composition {
        first: 0x41,
        second: 0x301,
        result: 0xC1,
    },
    Composition {
        first: 0x41,
        second: 0x302,
        result: 0xC2,
    },
    Composition {
        first: 0x41,
        second: 0x303,
        result: 0xC3,
    },
    Composition {
        first: 0x41,
        second: 0x304,
        result: 0x100,
    },
    Composition {
        first: 0x41,
        second: 0x306,
        result: 0x102,
    },
    Composition {
        first: 0x41,
        second: 0x307,
        result: 0x226,
    },
    Composition {
        first: 0x41,
        second: 0x308,
        result: 0xC4,
    },
    Composition {
        first: 0x41,
        second: 0x309,
        result: 0x1EA2,
    },
    Composition {
        first: 0x41,
        second: 0x30A,
        result: 0xC5,
    },
    Composition {
        first: 0x41,
        second: 0x30C,
        result: 0x1CD,
    },
    Composition {
        first: 0x41,
        second: 0x30F,
        result: 0x200,
    },
    Composition {
        first: 0x41,
        second: 0x311,
        result: 0x202,
    },
    Composition {
        first: 0x41,
        second: 0x323,
        result: 0x1EA0,
    },
    Composition {
        first: 0x41,
        second: 0x325,
        result: 0x1E00,
    },
    Composition {
        first: 0x41,
        second: 0x328,
        result: 0x104,
    },
    Composition {
        first: 0x42,
        second: 0x307,
        result: 0x1E02,
    },
    Composition {
        first: 0x42,
        second: 0x323,
        result: 0x1E04,
    },
    Composition {
        first: 0x42,
        second: 0x331,
        result: 0x1E06,
    },
    Composition {
        first: 0x43,
        second: 0x301,
        result: 0x106,
    },
    Composition {
        first: 0x43,
        second: 0x302,
        result: 0x108,
    },
    Composition {
        first: 0x43,
        second: 0x307,
        result: 0x10A,
    },
    Composition {
        first: 0x43,
        second: 0x30C,
        result: 0x10C,
    },
    Composition {
        first: 0x43,
        second: 0x327,
        result: 0xC7,
    },
    Composition {
        first: 0x44,
        second: 0x307,
        result: 0x1E0A,
    },
    Composition {
        first: 0x44,
        second: 0x30C,
        result: 0x10E,
    },
    Composition {
        first: 0x44,
        second: 0x323,
        result: 0x1E0C,
    },
    Composition {
        first: 0x44,
        second: 0x327,
        result: 0x1E10,
    },
    Composition {
        first: 0x44,
        second: 0x32D,
        result: 0x1E12,
    },
    Composition {
        first: 0x44,
        second: 0x331,
        result: 0x1E0E,
    },
    Composition {
        first: 0x45,
        second: 0x300,
        result: 0xC8,
    },
    Composition {
        first: 0x45,
        second: 0x301,
        result: 0xC9,
    },
    Composition {
        first: 0x45,
        second: 0x302,
        result: 0xCA,
    },
    Composition {
        first: 0x45,
        second: 0x303,
        result: 0x1EBC,
    },
    Composition {
        first: 0x45,
        second: 0x304,
        result: 0x112,
    },
    Composition {
        first: 0x45,
        second: 0x306,
        result: 0x114,
    },
    Composition {
        first: 0x45,
        second: 0x307,
        result: 0x116,
    },
    Composition {
        first: 0x45,
        second: 0x308,
        result: 0xCB,
    },
    Composition {
        first: 0x45,
        second: 0x309,
        result: 0x1EBA,
    },
    Composition {
        first: 0x45,
        second: 0x30C,
        result: 0x11A,
    },
    Composition {
        first: 0x45,
        second: 0x30F,
        result: 0x204,
    },
    Composition {
        first: 0x45,
        second: 0x311,
        result: 0x206,
    },
    Composition {
        first: 0x45,
        second: 0x323,
        result: 0x1EB8,
    },
    Composition {
        first: 0x45,
        second: 0x327,
        result: 0x228,
    },
    Composition {
        first: 0x45,
        second: 0x328,
        result: 0x118,
    },
    Composition {
        first: 0x45,
        second: 0x32D,
        result: 0x1E18,
    },
    Composition {
        first: 0x45,
        second: 0x330,
        result: 0x1E1A,
    },
    Composition {
        first: 0x46,
        second: 0x307,
        result: 0x1E1E,
    },
    Composition {
        first: 0x47,
        second: 0x301,
        result: 0x1F4,
    },
    Composition {
        first: 0x47,
        second: 0x302,
        result: 0x11C,
    },
    Composition {
        first: 0x47,
        second: 0x304,
        result: 0x1E20,
    },
    Composition {
        first: 0x47,
        second: 0x306,
        result: 0x11E,
    },
    Composition {
        first: 0x47,
        second: 0x307,
        result: 0x120,
    },
    Composition {
        first: 0x47,
        second: 0x30C,
        result: 0x1E6,
    },
    Composition {
        first: 0x47,
        second: 0x327,
        result: 0x122,
    },
    Composition {
        first: 0x48,
        second: 0x302,
        result: 0x124,
    },
    Composition {
        first: 0x48,
        second: 0x307,
        result: 0x1E22,
    },
    Composition {
        first: 0x48,
        second: 0x308,
        result: 0x1E26,
    },
    Composition {
        first: 0x48,
        second: 0x30C,
        result: 0x21E,
    },
    Composition {
        first: 0x48,
        second: 0x323,
        result: 0x1E24,
    },
    Composition {
        first: 0x48,
        second: 0x327,
        result: 0x1E28,
    },
    Composition {
        first: 0x48,
        second: 0x32E,
        result: 0x1E2A,
    },
    Composition {
        first: 0x49,
        second: 0x300,
        result: 0xCC,
    },
    Composition {
        first: 0x49,
        second: 0x301,
        result: 0xCD,
    },
    Composition {
        first: 0x49,
        second: 0x302,
        result: 0xCE,
    },
    Composition {
        first: 0x49,
        second: 0x303,
        result: 0x128,
    },
    Composition {
        first: 0x49,
        second: 0x304,
        result: 0x12A,
    },
    Composition {
        first: 0x49,
        second: 0x306,
        result: 0x12C,
    },
    Composition {
        first: 0x49,
        second: 0x307,
        result: 0x130,
    },
    Composition {
        first: 0x49,
        second: 0x308,
        result: 0xCF,
    },
    Composition {
        first: 0x49,
        second: 0x309,
        result: 0x1EC8,
    },
    Composition {
        first: 0x49,
        second: 0x30C,
        result: 0x1CF,
    },
    Composition {
        first: 0x49,
        second: 0x30F,
        result: 0x208,
    },
    Composition {
        first: 0x49,
        second: 0x311,
        result: 0x20A,
    },
    Composition {
        first: 0x49,
        second: 0x323,
        result: 0x1ECA,
    },
    Composition {
        first: 0x49,
        second: 0x328,
        result: 0x12E,
    },
    Composition {
        first: 0x49,
        second: 0x330,
        result: 0x1E2C,
    },
    Composition {
        first: 0x4A,
        second: 0x302,
        result: 0x134,
    },
    Composition {
        first: 0x4B,
        second: 0x301,
        result: 0x1E30,
    },
    Composition {
        first: 0x4B,
        second: 0x30C,
        result: 0x1E8,
    },
    Composition {
        first: 0x4B,
        second: 0x323,
        result: 0x1E32,
    },
    Composition {
        first: 0x4B,
        second: 0x327,
        result: 0x136,
    },
    Composition {
        first: 0x4B,
        second: 0x331,
        result: 0x1E34,
    },
    Composition {
        first: 0x4C,
        second: 0x301,
        result: 0x139,
    },
    Composition {
        first: 0x4C,
        second: 0x30C,
        result: 0x13D,
    },
    Composition {
        first: 0x4C,
        second: 0x323,
        result: 0x1E36,
    },
    Composition {
        first: 0x4C,
        second: 0x327,
        result: 0x13B,
    },
    Composition {
        first: 0x4C,
        second: 0x32D,
        result: 0x1E3C,
    },
    Composition {
        first: 0x4C,
        second: 0x331,
        result: 0x1E3A,
    },
    Composition {
        first: 0x4D,
        second: 0x301,
        result: 0x1E3E,
    },
    Composition {
        first: 0x4D,
        second: 0x307,
        result: 0x1E40,
    },
    Composition {
        first: 0x4D,
        second: 0x323,
        result: 0x1E42,
    },
    Composition {
        first: 0x4E,
        second: 0x300,
        result: 0x1F8,
    },
    Composition {
        first: 0x4E,
        second: 0x301,
        result: 0x143,
    },
    Composition {
        first: 0x4E,
        second: 0x303,
        result: 0xD1,
    },
    Composition {
        first: 0x4E,
        second: 0x307,
        result: 0x1E44,
    },
    Composition {
        first: 0x4E,
        second: 0x30C,
        result: 0x147,
    },
    Composition {
        first: 0x4E,
        second: 0x323,
        result: 0x1E46,
    },
    Composition {
        first: 0x4E,
        second: 0x327,
        result: 0x145,
    },
    Composition {
        first: 0x4E,
        second: 0x32D,
        result: 0x1E4A,
    },
    Composition {
        first: 0x4E,
        second: 0x331,
        result: 0x1E48,
    },
    Composition {
        first: 0x4F,
        second: 0x300,
        result: 0xD2,
    },
    Composition {
        first: 0x4F,
        second: 0x301,
        result: 0xD3,
    },
    Composition {
        first: 0x4F,
        second: 0x302,
        result: 0xD4,
    },
    Composition {
        first: 0x4F,
        second: 0x303,
        result: 0xD5,
    },
    Composition {
        first: 0x4F,
        second: 0x304,
        result: 0x14C,
    },
    Composition {
        first: 0x4F,
        second: 0x306,
        result: 0x14E,
    },
    Composition {
        first: 0x4F,
        second: 0x307,
        result: 0x22E,
    },
    Composition {
        first: 0x4F,
        second: 0x308,
        result: 0xD6,
    },
    Composition {
        first: 0x4F,
        second: 0x309,
        result: 0x1ECE,
    },
    Composition {
        first: 0x4F,
        second: 0x30B,
        result: 0x150,
    },
    Composition {
        first: 0x4F,
        second: 0x30C,
        result: 0x1D1,
    },
    Composition {
        first: 0x4F,
        second: 0x30F,
        result: 0x20C,
    },
    Composition {
        first: 0x4F,
        second: 0x311,
        result: 0x20E,
    },
    Composition {
        first: 0x4F,
        second: 0x31B,
        result: 0x1A0,
    },
    Composition {
        first: 0x4F,
        second: 0x323,
        result: 0x1ECC,
    },
    Composition {
        first: 0x4F,
        second: 0x328,
        result: 0x1EA,
    },
    Composition {
        first: 0x50,
        second: 0x301,
        result: 0x1E54,
    },
    Composition {
        first: 0x50,
        second: 0x307,
        result: 0x1E56,
    },
    Composition {
        first: 0x52,
        second: 0x301,
        result: 0x154,
    },
    Composition {
        first: 0x52,
        second: 0x307,
        result: 0x1E58,
    },
    Composition {
        first: 0x52,
        second: 0x30C,
        result: 0x158,
    },
    Composition {
        first: 0x52,
        second: 0x30F,
        result: 0x210,
    },
    Composition {
        first: 0x52,
        second: 0x311,
        result: 0x212,
    },
    Composition {
        first: 0x52,
        second: 0x323,
        result: 0x1E5A,
    },
    Composition {
        first: 0x52,
        second: 0x327,
        result: 0x156,
    },
    Composition {
        first: 0x52,
        second: 0x331,
        result: 0x1E5E,
    },
    Composition {
        first: 0x53,
        second: 0x301,
        result: 0x15A,
    },
    Composition {
        first: 0x53,
        second: 0x302,
        result: 0x15C,
    },
    Composition {
        first: 0x53,
        second: 0x307,
        result: 0x1E60,
    },
    Composition {
        first: 0x53,
        second: 0x30C,
        result: 0x160,
    },
    Composition {
        first: 0x53,
        second: 0x323,
        result: 0x1E62,
    },
    Composition {
        first: 0x53,
        second: 0x326,
        result: 0x218,
    },
    Composition {
        first: 0x53,
        second: 0x327,
        result: 0x15E,
    },
    Composition {
        first: 0x54,
        second: 0x307,
        result: 0x1E6A,
    },
    Composition {
        first: 0x54,
        second: 0x30C,
        result: 0x164,
    },
    Composition {
        first: 0x54,
        second: 0x323,
        result: 0x1E6C,
    },
    Composition {
        first: 0x54,
        second: 0x326,
        result: 0x21A,
    },
    Composition {
        first: 0x54,
        second: 0x327,
        result: 0x162,
    },
    Composition {
        first: 0x54,
        second: 0x32D,
        result: 0x1E70,
    },
    Composition {
        first: 0x54,
        second: 0x331,
        result: 0x1E6E,
    },
    Composition {
        first: 0x55,
        second: 0x300,
        result: 0xD9,
    },
    Composition {
        first: 0x55,
        second: 0x301,
        result: 0xDA,
    },
    Composition {
        first: 0x55,
        second: 0x302,
        result: 0xDB,
    },
    Composition {
        first: 0x55,
        second: 0x303,
        result: 0x168,
    },
    Composition {
        first: 0x55,
        second: 0x304,
        result: 0x16A,
    },
    Composition {
        first: 0x55,
        second: 0x306,
        result: 0x16C,
    },
    Composition {
        first: 0x55,
        second: 0x308,
        result: 0xDC,
    },
    Composition {
        first: 0x55,
        second: 0x309,
        result: 0x1EE6,
    },
    Composition {
        first: 0x55,
        second: 0x30A,
        result: 0x16E,
    },
    Composition {
        first: 0x55,
        second: 0x30B,
        result: 0x170,
    },
    Composition {
        first: 0x55,
        second: 0x30C,
        result: 0x1D3,
    },
    Composition {
        first: 0x55,
        second: 0x30F,
        result: 0x214,
    },
    Composition {
        first: 0x55,
        second: 0x311,
        result: 0x216,
    },
    Composition {
        first: 0x55,
        second: 0x31B,
        result: 0x1AF,
    },
    Composition {
        first: 0x55,
        second: 0x323,
        result: 0x1EE4,
    },
    Composition {
        first: 0x55,
        second: 0x324,
        result: 0x1E72,
    },
    Composition {
        first: 0x55,
        second: 0x328,
        result: 0x172,
    },
    Composition {
        first: 0x55,
        second: 0x32D,
        result: 0x1E76,
    },
    Composition {
        first: 0x55,
        second: 0x330,
        result: 0x1E74,
    },
    Composition {
        first: 0x56,
        second: 0x303,
        result: 0x1E7C,
    },
    Composition {
        first: 0x56,
        second: 0x323,
        result: 0x1E7E,
    },
    Composition {
        first: 0x57,
        second: 0x300,
        result: 0x1E80,
    },
    Composition {
        first: 0x57,
        second: 0x301,
        result: 0x1E82,
    },
    Composition {
        first: 0x57,
        second: 0x302,
        result: 0x174,
    },
    Composition {
        first: 0x57,
        second: 0x307,
        result: 0x1E86,
    },
    Composition {
        first: 0x57,
        second: 0x308,
        result: 0x1E84,
    },
    Composition {
        first: 0x57,
        second: 0x323,
        result: 0x1E88,
    },
    Composition {
        first: 0x58,
        second: 0x307,
        result: 0x1E8A,
    },
    Composition {
        first: 0x58,
        second: 0x308,
        result: 0x1E8C,
    },
    Composition {
        first: 0x59,
        second: 0x300,
        result: 0x1EF2,
    },
    Composition {
        first: 0x59,
        second: 0x301,
        result: 0xDD,
    },
    Composition {
        first: 0x59,
        second: 0x302,
        result: 0x176,
    },
    Composition {
        first: 0x59,
        second: 0x303,
        result: 0x1EF8,
    },
    Composition {
        first: 0x59,
        second: 0x304,
        result: 0x232,
    },
    Composition {
        first: 0x59,
        second: 0x307,
        result: 0x1E8E,
    },
    Composition {
        first: 0x59,
        second: 0x308,
        result: 0x178,
    },
    Composition {
        first: 0x59,
        second: 0x309,
        result: 0x1EF6,
    },
    Composition {
        first: 0x59,
        second: 0x323,
        result: 0x1EF4,
    },
    Composition {
        first: 0x5A,
        second: 0x301,
        result: 0x179,
    },
    Composition {
        first: 0x5A,
        second: 0x302,
        result: 0x1E90,
    },
    Composition {
        first: 0x5A,
        second: 0x307,
        result: 0x17B,
    },
    Composition {
        first: 0x5A,
        second: 0x30C,
        result: 0x17D,
    },
    Composition {
        first: 0x5A,
        second: 0x323,
        result: 0x1E92,
    },
    Composition {
        first: 0x5A,
        second: 0x331,
        result: 0x1E94,
    },
    Composition {
        first: 0x61,
        second: 0x300,
        result: 0xE0,
    },
    Composition {
        first: 0x61,
        second: 0x301,
        result: 0xE1,
    },
    Composition {
        first: 0x61,
        second: 0x302,
        result: 0xE2,
    },
    Composition {
        first: 0x61,
        second: 0x303,
        result: 0xE3,
    },
    Composition {
        first: 0x61,
        second: 0x304,
        result: 0x101,
    },
    Composition {
        first: 0x61,
        second: 0x306,
        result: 0x103,
    },
    Composition {
        first: 0x61,
        second: 0x307,
        result: 0x227,
    },
    Composition {
        first: 0x61,
        second: 0x308,
        result: 0xE4,
    },
    Composition {
        first: 0x61,
        second: 0x309,
        result: 0x1EA3,
    },
    Composition {
        first: 0x61,
        second: 0x30A,
        result: 0xE5,
    },
    Composition {
        first: 0x61,
        second: 0x30C,
        result: 0x1CE,
    },
    Composition {
        first: 0x61,
        second: 0x30F,
        result: 0x201,
    },
    Composition {
        first: 0x61,
        second: 0x311,
        result: 0x203,
    },
    Composition {
        first: 0x61,
        second: 0x323,
        result: 0x1EA1,
    },
    Composition {
        first: 0x61,
        second: 0x325,
        result: 0x1E01,
    },
    Composition {
        first: 0x61,
        second: 0x328,
        result: 0x105,
    },
    Composition {
        first: 0x62,
        second: 0x307,
        result: 0x1E03,
    },
    Composition {
        first: 0x62,
        second: 0x323,
        result: 0x1E05,
    },
    Composition {
        first: 0x62,
        second: 0x331,
        result: 0x1E07,
    },
    Composition {
        first: 0x63,
        second: 0x301,
        result: 0x107,
    },
    Composition {
        first: 0x63,
        second: 0x302,
        result: 0x109,
    },
    Composition {
        first: 0x63,
        second: 0x307,
        result: 0x10B,
    },
    Composition {
        first: 0x63,
        second: 0x30C,
        result: 0x10D,
    },
    Composition {
        first: 0x63,
        second: 0x327,
        result: 0xE7,
    },
    Composition {
        first: 0x64,
        second: 0x307,
        result: 0x1E0B,
    },
    Composition {
        first: 0x64,
        second: 0x30C,
        result: 0x10F,
    },
    Composition {
        first: 0x64,
        second: 0x323,
        result: 0x1E0D,
    },
    Composition {
        first: 0x64,
        second: 0x327,
        result: 0x1E11,
    },
    Composition {
        first: 0x64,
        second: 0x32D,
        result: 0x1E13,
    },
    Composition {
        first: 0x64,
        second: 0x331,
        result: 0x1E0F,
    },
    Composition {
        first: 0x65,
        second: 0x300,
        result: 0xE8,
    },
    Composition {
        first: 0x65,
        second: 0x301,
        result: 0xE9,
    },
    Composition {
        first: 0x65,
        second: 0x302,
        result: 0xEA,
    },
    Composition {
        first: 0x65,
        second: 0x303,
        result: 0x1EBD,
    },
    Composition {
        first: 0x65,
        second: 0x304,
        result: 0x113,
    },
    Composition {
        first: 0x65,
        second: 0x306,
        result: 0x115,
    },
    Composition {
        first: 0x65,
        second: 0x307,
        result: 0x117,
    },
    Composition {
        first: 0x65,
        second: 0x308,
        result: 0xEB,
    },
    Composition {
        first: 0x65,
        second: 0x309,
        result: 0x1EBB,
    },
    Composition {
        first: 0x65,
        second: 0x30C,
        result: 0x11B,
    },
    Composition {
        first: 0x65,
        second: 0x30F,
        result: 0x205,
    },
    Composition {
        first: 0x65,
        second: 0x311,
        result: 0x207,
    },
    Composition {
        first: 0x65,
        second: 0x323,
        result: 0x1EB9,
    },
    Composition {
        first: 0x65,
        second: 0x327,
        result: 0x229,
    },
    Composition {
        first: 0x65,
        second: 0x328,
        result: 0x119,
    },
    Composition {
        first: 0x65,
        second: 0x32D,
        result: 0x1E19,
    },
    Composition {
        first: 0x65,
        second: 0x330,
        result: 0x1E1B,
    },
    Composition {
        first: 0x66,
        second: 0x307,
        result: 0x1E1F,
    },
    Composition {
        first: 0x67,
        second: 0x301,
        result: 0x1F5,
    },
    Composition {
        first: 0x67,
        second: 0x302,
        result: 0x11D,
    },
    Composition {
        first: 0x67,
        second: 0x304,
        result: 0x1E21,
    },
    Composition {
        first: 0x67,
        second: 0x306,
        result: 0x11F,
    },
    Composition {
        first: 0x67,
        second: 0x307,
        result: 0x121,
    },
    Composition {
        first: 0x67,
        second: 0x30C,
        result: 0x1E7,
    },
    Composition {
        first: 0x67,
        second: 0x327,
        result: 0x123,
    },
    Composition {
        first: 0x68,
        second: 0x302,
        result: 0x125,
    },
    Composition {
        first: 0x68,
        second: 0x307,
        result: 0x1E23,
    },
    Composition {
        first: 0x68,
        second: 0x308,
        result: 0x1E27,
    },
    Composition {
        first: 0x68,
        second: 0x30C,
        result: 0x21F,
    },
    Composition {
        first: 0x68,
        second: 0x323,
        result: 0x1E25,
    },
    Composition {
        first: 0x68,
        second: 0x327,
        result: 0x1E29,
    },
    Composition {
        first: 0x68,
        second: 0x32E,
        result: 0x1E2B,
    },
    Composition {
        first: 0x68,
        second: 0x331,
        result: 0x1E96,
    },
    Composition {
        first: 0x69,
        second: 0x300,
        result: 0xEC,
    },
    Composition {
        first: 0x69,
        second: 0x301,
        result: 0xED,
    },
    Composition {
        first: 0x69,
        second: 0x302,
        result: 0xEE,
    },
    Composition {
        first: 0x69,
        second: 0x303,
        result: 0x129,
    },
    Composition {
        first: 0x69,
        second: 0x304,
        result: 0x12B,
    },
    Composition {
        first: 0x69,
        second: 0x306,
        result: 0x12D,
    },
    Composition {
        first: 0x69,
        second: 0x308,
        result: 0xEF,
    },
    Composition {
        first: 0x69,
        second: 0x309,
        result: 0x1EC9,
    },
    Composition {
        first: 0x69,
        second: 0x30C,
        result: 0x1D0,
    },
    Composition {
        first: 0x69,
        second: 0x30F,
        result: 0x209,
    },
    Composition {
        first: 0x69,
        second: 0x311,
        result: 0x20B,
    },
    Composition {
        first: 0x69,
        second: 0x323,
        result: 0x1ECB,
    },
    Composition {
        first: 0x69,
        second: 0x328,
        result: 0x12F,
    },
    Composition {
        first: 0x69,
        second: 0x330,
        result: 0x1E2D,
    },
    Composition {
        first: 0x6A,
        second: 0x302,
        result: 0x135,
    },
    Composition {
        first: 0x6A,
        second: 0x30C,
        result: 0x1F0,
    },
    Composition {
        first: 0x6B,
        second: 0x301,
        result: 0x1E31,
    },
    Composition {
        first: 0x6B,
        second: 0x30C,
        result: 0x1E9,
    },
    Composition {
        first: 0x6B,
        second: 0x323,
        result: 0x1E33,
    },
    Composition {
        first: 0x6B,
        second: 0x327,
        result: 0x137,
    },
    Composition {
        first: 0x6B,
        second: 0x331,
        result: 0x1E35,
    },
    Composition {
        first: 0x6C,
        second: 0x301,
        result: 0x13A,
    },
    Composition {
        first: 0x6C,
        second: 0x30C,
        result: 0x13E,
    },
    Composition {
        first: 0x6C,
        second: 0x323,
        result: 0x1E37,
    },
    Composition {
        first: 0x6C,
        second: 0x327,
        result: 0x13C,
    },
    Composition {
        first: 0x6C,
        second: 0x32D,
        result: 0x1E3D,
    },
    Composition {
        first: 0x6C,
        second: 0x331,
        result: 0x1E3B,
    },
    Composition {
        first: 0x6D,
        second: 0x301,
        result: 0x1E3F,
    },
    Composition {
        first: 0x6D,
        second: 0x307,
        result: 0x1E41,
    },
    Composition {
        first: 0x6D,
        second: 0x323,
        result: 0x1E43,
    },
    Composition {
        first: 0x6E,
        second: 0x300,
        result: 0x1F9,
    },
    Composition {
        first: 0x6E,
        second: 0x301,
        result: 0x144,
    },
    Composition {
        first: 0x6E,
        second: 0x303,
        result: 0xF1,
    },
    Composition {
        first: 0x6E,
        second: 0x307,
        result: 0x1E45,
    },
    Composition {
        first: 0x6E,
        second: 0x30C,
        result: 0x148,
    },
    Composition {
        first: 0x6E,
        second: 0x323,
        result: 0x1E47,
    },
    Composition {
        first: 0x6E,
        second: 0x327,
        result: 0x146,
    },
    Composition {
        first: 0x6E,
        second: 0x32D,
        result: 0x1E4B,
    },
    Composition {
        first: 0x6E,
        second: 0x331,
        result: 0x1E49,
    },
    Composition {
        first: 0x6F,
        second: 0x300,
        result: 0xF2,
    },
    Composition {
        first: 0x6F,
        second: 0x301,
        result: 0xF3,
    },
    Composition {
        first: 0x6F,
        second: 0x302,
        result: 0xF4,
    },
    Composition {
        first: 0x6F,
        second: 0x303,
        result: 0xF5,
    },
    Composition {
        first: 0x6F,
        second: 0x304,
        result: 0x14D,
    },
    Composition {
        first: 0x6F,
        second: 0x306,
        result: 0x14F,
    },
    Composition {
        first: 0x6F,
        second: 0x307,
        result: 0x22F,
    },
    Composition {
        first: 0x6F,
        second: 0x308,
        result: 0xF6,
    },
    Composition {
        first: 0x6F,
        second: 0x309,
        result: 0x1ECF,
    },
    Composition {
        first: 0x6F,
        second: 0x30B,
        result: 0x151,
    },
    Composition {
        first: 0x6F,
        second: 0x30C,
        result: 0x1D2,
    },
    Composition {
        first: 0x6F,
        second: 0x30F,
        result: 0x20D,
    },
    Composition {
        first: 0x6F,
        second: 0x311,
        result: 0x20F,
    },
    Composition {
        first: 0x6F,
        second: 0x31B,
        result: 0x1A1,
    },
    Composition {
        first: 0x6F,
        second: 0x323,
        result: 0x1ECD,
    },
    Composition {
        first: 0x6F,
        second: 0x328,
        result: 0x1EB,
    },
    Composition {
        first: 0x70,
        second: 0x301,
        result: 0x1E55,
    },
    Composition {
        first: 0x70,
        second: 0x307,
        result: 0x1E57,
    },
    Composition {
        first: 0x72,
        second: 0x301,
        result: 0x155,
    },
    Composition {
        first: 0x72,
        second: 0x307,
        result: 0x1E59,
    },
    Composition {
        first: 0x72,
        second: 0x30C,
        result: 0x159,
    },
    Composition {
        first: 0x72,
        second: 0x30F,
        result: 0x211,
    },
    Composition {
        first: 0x72,
        second: 0x311,
        result: 0x213,
    },
    Composition {
        first: 0x72,
        second: 0x323,
        result: 0x1E5B,
    },
    Composition {
        first: 0x72,
        second: 0x327,
        result: 0x157,
    },
    Composition {
        first: 0x72,
        second: 0x331,
        result: 0x1E5F,
    },
    Composition {
        first: 0x73,
        second: 0x301,
        result: 0x15B,
    },
    Composition {
        first: 0x73,
        second: 0x302,
        result: 0x15D,
    },
    Composition {
        first: 0x73,
        second: 0x307,
        result: 0x1E61,
    },
    Composition {
        first: 0x73,
        second: 0x30C,
        result: 0x161,
    },
    Composition {
        first: 0x73,
        second: 0x323,
        result: 0x1E63,
    },
    Composition {
        first: 0x73,
        second: 0x326,
        result: 0x219,
    },
    Composition {
        first: 0x73,
        second: 0x327,
        result: 0x15F,
    },
    Composition {
        first: 0x74,
        second: 0x307,
        result: 0x1E6B,
    },
    Composition {
        first: 0x74,
        second: 0x308,
        result: 0x1E97,
    },
    Composition {
        first: 0x74,
        second: 0x30C,
        result: 0x165,
    },
    Composition {
        first: 0x74,
        second: 0x323,
        result: 0x1E6D,
    },
    Composition {
        first: 0x74,
        second: 0x326,
        result: 0x21B,
    },
    Composition {
        first: 0x74,
        second: 0x327,
        result: 0x163,
    },
    Composition {
        first: 0x74,
        second: 0x32D,
        result: 0x1E71,
    },
    Composition {
        first: 0x74,
        second: 0x331,
        result: 0x1E6F,
    },
    Composition {
        first: 0x75,
        second: 0x300,
        result: 0xF9,
    },
    Composition {
        first: 0x75,
        second: 0x301,
        result: 0xFA,
    },
    Composition {
        first: 0x75,
        second: 0x302,
        result: 0xFB,
    },
    Composition {
        first: 0x75,
        second: 0x303,
        result: 0x169,
    },
    Composition {
        first: 0x75,
        second: 0x304,
        result: 0x16B,
    },
    Composition {
        first: 0x75,
        second: 0x306,
        result: 0x16D,
    },
    Composition {
        first: 0x75,
        second: 0x308,
        result: 0xFC,
    },
    Composition {
        first: 0x75,
        second: 0x309,
        result: 0x1EE7,
    },
    Composition {
        first: 0x75,
        second: 0x30A,
        result: 0x16F,
    },
    Composition {
        first: 0x75,
        second: 0x30B,
        result: 0x171,
    },
    Composition {
        first: 0x75,
        second: 0x30C,
        result: 0x1D4,
    },
    Composition {
        first: 0x75,
        second: 0x30F,
        result: 0x215,
    },
    Composition {
        first: 0x75,
        second: 0x311,
        result: 0x217,
    },
    Composition {
        first: 0x75,
        second: 0x31B,
        result: 0x1B0,
    },
    Composition {
        first: 0x75,
        second: 0x323,
        result: 0x1EE5,
    },
    Composition {
        first: 0x75,
        second: 0x324,
        result: 0x1E73,
    },
    Composition {
        first: 0x75,
        second: 0x328,
        result: 0x173,
    },
    Composition {
        first: 0x75,
        second: 0x32D,
        result: 0x1E77,
    },
    Composition {
        first: 0x75,
        second: 0x330,
        result: 0x1E75,
    },
    Composition {
        first: 0x76,
        second: 0x303,
        result: 0x1E7D,
    },
    Composition {
        first: 0x76,
        second: 0x323,
        result: 0x1E7F,
    },
    Composition {
        first: 0x77,
        second: 0x300,
        result: 0x1E81,
    },
    Composition {
        first: 0x77,
        second: 0x301,
        result: 0x1E83,
    },
    Composition {
        first: 0x77,
        second: 0x302,
        result: 0x175,
    },
    Composition {
        first: 0x77,
        second: 0x307,
        result: 0x1E87,
    },
    Composition {
        first: 0x77,
        second: 0x308,
        result: 0x1E85,
    },
    Composition {
        first: 0x77,
        second: 0x30A,
        result: 0x1E98,
    },
    Composition {
        first: 0x77,
        second: 0x323,
        result: 0x1E89,
    },
    Composition {
        first: 0x78,
        second: 0x307,
        result: 0x1E8B,
    },
    Composition {
        first: 0x78,
        second: 0x308,
        result: 0x1E8D,
    },
    Composition {
        first: 0x79,
        second: 0x300,
        result: 0x1EF3,
    },
    Composition {
        first: 0x79,
        second: 0x301,
        result: 0xFD,
    },
    Composition {
        first: 0x79,
        second: 0x302,
        result: 0x177,
    },
    Composition {
        first: 0x79,
        second: 0x303,
        result: 0x1EF9,
    },
    Composition {
        first: 0x79,
        second: 0x304,
        result: 0x233,
    },
    Composition {
        first: 0x79,
        second: 0x307,
        result: 0x1E8F,
    },
    Composition {
        first: 0x79,
        second: 0x308,
        result: 0xFF,
    },
    Composition {
        first: 0x79,
        second: 0x309,
        result: 0x1EF7,
    },
    Composition {
        first: 0x79,
        second: 0x30A,
        result: 0x1E99,
    },
    Composition {
        first: 0x79,
        second: 0x323,
        result: 0x1EF5,
    },
    Composition {
        first: 0x7A,
        second: 0x301,
        result: 0x17A,
    },
    Composition {
        first: 0x7A,
        second: 0x302,
        result: 0x1E91,
    },
    Composition {
        first: 0x7A,
        second: 0x307,
        result: 0x17C,
    },
    Composition {
        first: 0x7A,
        second: 0x30C,
        result: 0x17E,
    },
    Composition {
        first: 0x7A,
        second: 0x323,
        result: 0x1E93,
    },
    Composition {
        first: 0x7A,
        second: 0x331,
        result: 0x1E95,
    },
    Composition {
        first: 0xA8,
        second: 0x300,
        result: 0x1FED,
    },
    Composition {
        first: 0xA8,
        second: 0x301,
        result: 0x385,
    },
    Composition {
        first: 0xA8,
        second: 0x342,
        result: 0x1FC1,
    },
    Composition {
        first: 0xC2,
        second: 0x300,
        result: 0x1EA6,
    },
    Composition {
        first: 0xC2,
        second: 0x301,
        result: 0x1EA4,
    },
    Composition {
        first: 0xC2,
        second: 0x303,
        result: 0x1EAA,
    },
    Composition {
        first: 0xC2,
        second: 0x309,
        result: 0x1EA8,
    },
    Composition {
        first: 0xC4,
        second: 0x304,
        result: 0x1DE,
    },
    Composition {
        first: 0xC5,
        second: 0x301,
        result: 0x1FA,
    },
    Composition {
        first: 0xC6,
        second: 0x301,
        result: 0x1FC,
    },
    Composition {
        first: 0xC6,
        second: 0x304,
        result: 0x1E2,
    },
    Composition {
        first: 0xC7,
        second: 0x301,
        result: 0x1E08,
    },
    Composition {
        first: 0xCA,
        second: 0x300,
        result: 0x1EC0,
    },
    Composition {
        first: 0xCA,
        second: 0x301,
        result: 0x1EBE,
    },
    Composition {
        first: 0xCA,
        second: 0x303,
        result: 0x1EC4,
    },
    Composition {
        first: 0xCA,
        second: 0x309,
        result: 0x1EC2,
    },
    Composition {
        first: 0xCF,
        second: 0x301,
        result: 0x1E2E,
    },
    Composition {
        first: 0xD4,
        second: 0x300,
        result: 0x1ED2,
    },
    Composition {
        first: 0xD4,
        second: 0x301,
        result: 0x1ED0,
    },
    Composition {
        first: 0xD4,
        second: 0x303,
        result: 0x1ED6,
    },
    Composition {
        first: 0xD4,
        second: 0x309,
        result: 0x1ED4,
    },
    Composition {
        first: 0xD5,
        second: 0x301,
        result: 0x1E4C,
    },
    Composition {
        first: 0xD5,
        second: 0x304,
        result: 0x22C,
    },
    Composition {
        first: 0xD5,
        second: 0x308,
        result: 0x1E4E,
    },
    Composition {
        first: 0xD6,
        second: 0x304,
        result: 0x22A,
    },
    Composition {
        first: 0xD8,
        second: 0x301,
        result: 0x1FE,
    },
    Composition {
        first: 0xDC,
        second: 0x300,
        result: 0x1DB,
    },
    Composition {
        first: 0xDC,
        second: 0x301,
        result: 0x1D7,
    },
    Composition {
        first: 0xDC,
        second: 0x304,
        result: 0x1D5,
    },
    Composition {
        first: 0xDC,
        second: 0x30C,
        result: 0x1D9,
    },
    Composition {
        first: 0xE2,
        second: 0x300,
        result: 0x1EA7,
    },
    Composition {
        first: 0xE2,
        second: 0x301,
        result: 0x1EA5,
    },
    Composition {
        first: 0xE2,
        second: 0x303,
        result: 0x1EAB,
    },
    Composition {
        first: 0xE2,
        second: 0x309,
        result: 0x1EA9,
    },
    Composition {
        first: 0xE4,
        second: 0x304,
        result: 0x1DF,
    },
    Composition {
        first: 0xE5,
        second: 0x301,
        result: 0x1FB,
    },
    Composition {
        first: 0xE6,
        second: 0x301,
        result: 0x1FD,
    },
    Composition {
        first: 0xE6,
        second: 0x304,
        result: 0x1E3,
    },
    Composition {
        first: 0xE7,
        second: 0x301,
        result: 0x1E09,
    },
    Composition {
        first: 0xEA,
        second: 0x300,
        result: 0x1EC1,
    },
    Composition {
        first: 0xEA,
        second: 0x301,
        result: 0x1EBF,
    },
    Composition {
        first: 0xEA,
        second: 0x303,
        result: 0x1EC5,
    },
    Composition {
        first: 0xEA,
        second: 0x309,
        result: 0x1EC3,
    },
    Composition {
        first: 0xEF,
        second: 0x301,
        result: 0x1E2F,
    },
    Composition {
        first: 0xF4,
        second: 0x300,
        result: 0x1ED3,
    },
    Composition {
        first: 0xF4,
        second: 0x301,
        result: 0x1ED1,
    },
    Composition {
        first: 0xF4,
        second: 0x303,
        result: 0x1ED7,
    },
    Composition {
        first: 0xF4,
        second: 0x309,
        result: 0x1ED5,
    },
    Composition {
        first: 0xF5,
        second: 0x301,
        result: 0x1E4D,
    },
    Composition {
        first: 0xF5,
        second: 0x304,
        result: 0x22D,
    },
    Composition {
        first: 0xF5,
        second: 0x308,
        result: 0x1E4F,
    },
    Composition {
        first: 0xF6,
        second: 0x304,
        result: 0x22B,
    },
    Composition {
        first: 0xF8,
        second: 0x301,
        result: 0x1FF,
    },
    Composition {
        first: 0xFC,
        second: 0x300,
        result: 0x1DC,
    },
    Composition {
        first: 0xFC,
        second: 0x301,
        result: 0x1D8,
    },
    Composition {
        first: 0xFC,
        second: 0x304,
        result: 0x1D6,
    },
    Composition {
        first: 0xFC,
        second: 0x30C,
        result: 0x1DA,
    },
    Composition {
        first: 0x102,
        second: 0x300,
        result: 0x1EB0,
    },
    Composition {
        first: 0x102,
        second: 0x301,
        result: 0x1EAE,
    },
    Composition {
        first: 0x102,
        second: 0x303,
        result: 0x1EB4,
    },
    Composition {
        first: 0x102,
        second: 0x309,
        result: 0x1EB2,
    },
    Composition {
        first: 0x103,
        second: 0x300,
        result: 0x1EB1,
    },
    Composition {
        first: 0x103,
        second: 0x301,
        result: 0x1EAF,
    },
    Composition {
        first: 0x103,
        second: 0x303,
        result: 0x1EB5,
    },
    Composition {
        first: 0x103,
        second: 0x309,
        result: 0x1EB3,
    },
    Composition {
        first: 0x112,
        second: 0x300,
        result: 0x1E14,
    },
    Composition {
        first: 0x112,
        second: 0x301,
        result: 0x1E16,
    },
    Composition {
        first: 0x113,
        second: 0x300,
        result: 0x1E15,
    },
    Composition {
        first: 0x113,
        second: 0x301,
        result: 0x1E17,
    },
    Composition {
        first: 0x14C,
        second: 0x300,
        result: 0x1E50,
    },
    Composition {
        first: 0x14C,
        second: 0x301,
        result: 0x1E52,
    },
    Composition {
        first: 0x14D,
        second: 0x300,
        result: 0x1E51,
    },
    Composition {
        first: 0x14D,
        second: 0x301,
        result: 0x1E53,
    },
    Composition {
        first: 0x15A,
        second: 0x307,
        result: 0x1E64,
    },
    Composition {
        first: 0x15B,
        second: 0x307,
        result: 0x1E65,
    },
    Composition {
        first: 0x160,
        second: 0x307,
        result: 0x1E66,
    },
    Composition {
        first: 0x161,
        second: 0x307,
        result: 0x1E67,
    },
    Composition {
        first: 0x168,
        second: 0x301,
        result: 0x1E78,
    },
    Composition {
        first: 0x169,
        second: 0x301,
        result: 0x1E79,
    },
    Composition {
        first: 0x16A,
        second: 0x308,
        result: 0x1E7A,
    },
    Composition {
        first: 0x16B,
        second: 0x308,
        result: 0x1E7B,
    },
    Composition {
        first: 0x17F,
        second: 0x307,
        result: 0x1E9B,
    },
    Composition {
        first: 0x1A0,
        second: 0x300,
        result: 0x1EDC,
    },
    Composition {
        first: 0x1A0,
        second: 0x301,
        result: 0x1EDA,
    },
    Composition {
        first: 0x1A0,
        second: 0x303,
        result: 0x1EE0,
    },
    Composition {
        first: 0x1A0,
        second: 0x309,
        result: 0x1EDE,
    },
    Composition {
        first: 0x1A0,
        second: 0x323,
        result: 0x1EE2,
    },
    Composition {
        first: 0x1A1,
        second: 0x300,
        result: 0x1EDD,
    },
    Composition {
        first: 0x1A1,
        second: 0x301,
        result: 0x1EDB,
    },
    Composition {
        first: 0x1A1,
        second: 0x303,
        result: 0x1EE1,
    },
    Composition {
        first: 0x1A1,
        second: 0x309,
        result: 0x1EDF,
    },
    Composition {
        first: 0x1A1,
        second: 0x323,
        result: 0x1EE3,
    },
    Composition {
        first: 0x1AF,
        second: 0x300,
        result: 0x1EEA,
    },
    Composition {
        first: 0x1AF,
        second: 0x301,
        result: 0x1EE8,
    },
    Composition {
        first: 0x1AF,
        second: 0x303,
        result: 0x1EEE,
    },
    Composition {
        first: 0x1AF,
        second: 0x309,
        result: 0x1EEC,
    },
    Composition {
        first: 0x1AF,
        second: 0x323,
        result: 0x1EF0,
    },
    Composition {
        first: 0x1B0,
        second: 0x300,
        result: 0x1EEB,
    },
    Composition {
        first: 0x1B0,
        second: 0x301,
        result: 0x1EE9,
    },
    Composition {
        first: 0x1B0,
        second: 0x303,
        result: 0x1EEF,
    },
    Composition {
        first: 0x1B0,
        second: 0x309,
        result: 0x1EED,
    },
    Composition {
        first: 0x1B0,
        second: 0x323,
        result: 0x1EF1,
    },
    Composition {
        first: 0x1B7,
        second: 0x30C,
        result: 0x1EE,
    },
    Composition {
        first: 0x1EA,
        second: 0x304,
        result: 0x1EC,
    },
    Composition {
        first: 0x1EB,
        second: 0x304,
        result: 0x1ED,
    },
    Composition {
        first: 0x226,
        second: 0x304,
        result: 0x1E0,
    },
    Composition {
        first: 0x227,
        second: 0x304,
        result: 0x1E1,
    },
    Composition {
        first: 0x228,
        second: 0x306,
        result: 0x1E1C,
    },
    Composition {
        first: 0x229,
        second: 0x306,
        result: 0x1E1D,
    },
    Composition {
        first: 0x22E,
        second: 0x304,
        result: 0x230,
    },
    Composition {
        first: 0x22F,
        second: 0x304,
        result: 0x231,
    },
    Composition {
        first: 0x292,
        second: 0x30C,
        result: 0x1EF,
    },
    Composition {
        first: 0x391,
        second: 0x300,
        result: 0x1FBA,
    },
    Composition {
        first: 0x391,
        second: 0x301,
        result: 0x386,
    },
    Composition {
        first: 0x391,
        second: 0x304,
        result: 0x1FB9,
    },
    Composition {
        first: 0x391,
        second: 0x306,
        result: 0x1FB8,
    },
    Composition {
        first: 0x391,
        second: 0x313,
        result: 0x1F08,
    },
    Composition {
        first: 0x391,
        second: 0x314,
        result: 0x1F09,
    },
    Composition {
        first: 0x391,
        second: 0x345,
        result: 0x1FBC,
    },
    Composition {
        first: 0x395,
        second: 0x300,
        result: 0x1FC8,
    },
    Composition {
        first: 0x395,
        second: 0x301,
        result: 0x388,
    },
    Composition {
        first: 0x395,
        second: 0x313,
        result: 0x1F18,
    },
    Composition {
        first: 0x395,
        second: 0x314,
        result: 0x1F19,
    },
    Composition {
        first: 0x397,
        second: 0x300,
        result: 0x1FCA,
    },
    Composition {
        first: 0x397,
        second: 0x301,
        result: 0x389,
    },
    Composition {
        first: 0x397,
        second: 0x313,
        result: 0x1F28,
    },
    Composition {
        first: 0x397,
        second: 0x314,
        result: 0x1F29,
    },
    Composition {
        first: 0x397,
        second: 0x345,
        result: 0x1FCC,
    },
    Composition {
        first: 0x399,
        second: 0x300,
        result: 0x1FDA,
    },
    Composition {
        first: 0x399,
        second: 0x301,
        result: 0x38A,
    },
    Composition {
        first: 0x399,
        second: 0x304,
        result: 0x1FD9,
    },
    Composition {
        first: 0x399,
        second: 0x306,
        result: 0x1FD8,
    },
    Composition {
        first: 0x399,
        second: 0x308,
        result: 0x3AA,
    },
    Composition {
        first: 0x399,
        second: 0x313,
        result: 0x1F38,
    },
    Composition {
        first: 0x399,
        second: 0x314,
        result: 0x1F39,
    },
    Composition {
        first: 0x39F,
        second: 0x300,
        result: 0x1FF8,
    },
    Composition {
        first: 0x39F,
        second: 0x301,
        result: 0x38C,
    },
    Composition {
        first: 0x39F,
        second: 0x313,
        result: 0x1F48,
    },
    Composition {
        first: 0x39F,
        second: 0x314,
        result: 0x1F49,
    },
    Composition {
        first: 0x3A1,
        second: 0x314,
        result: 0x1FEC,
    },
    Composition {
        first: 0x3A5,
        second: 0x300,
        result: 0x1FEA,
    },
    Composition {
        first: 0x3A5,
        second: 0x301,
        result: 0x38E,
    },
    Composition {
        first: 0x3A5,
        second: 0x304,
        result: 0x1FE9,
    },
    Composition {
        first: 0x3A5,
        second: 0x306,
        result: 0x1FE8,
    },
    Composition {
        first: 0x3A5,
        second: 0x308,
        result: 0x3AB,
    },
    Composition {
        first: 0x3A5,
        second: 0x314,
        result: 0x1F59,
    },
    Composition {
        first: 0x3A9,
        second: 0x300,
        result: 0x1FFA,
    },
    Composition {
        first: 0x3A9,
        second: 0x301,
        result: 0x38F,
    },
    Composition {
        first: 0x3A9,
        second: 0x313,
        result: 0x1F68,
    },
    Composition {
        first: 0x3A9,
        second: 0x314,
        result: 0x1F69,
    },
    Composition {
        first: 0x3A9,
        second: 0x345,
        result: 0x1FFC,
    },
    Composition {
        first: 0x3AC,
        second: 0x345,
        result: 0x1FB4,
    },
    Composition {
        first: 0x3AE,
        second: 0x345,
        result: 0x1FC4,
    },
    Composition {
        first: 0x3B1,
        second: 0x300,
        result: 0x1F70,
    },
    Composition {
        first: 0x3B1,
        second: 0x301,
        result: 0x3AC,
    },
    Composition {
        first: 0x3B1,
        second: 0x304,
        result: 0x1FB1,
    },
    Composition {
        first: 0x3B1,
        second: 0x306,
        result: 0x1FB0,
    },
    Composition {
        first: 0x3B1,
        second: 0x313,
        result: 0x1F00,
    },
    Composition {
        first: 0x3B1,
        second: 0x314,
        result: 0x1F01,
    },
    Composition {
        first: 0x3B1,
        second: 0x342,
        result: 0x1FB6,
    },
    Composition {
        first: 0x3B1,
        second: 0x345,
        result: 0x1FB3,
    },
    Composition {
        first: 0x3B5,
        second: 0x300,
        result: 0x1F72,
    },
    Composition {
        first: 0x3B5,
        second: 0x301,
        result: 0x3AD,
    },
    Composition {
        first: 0x3B5,
        second: 0x313,
        result: 0x1F10,
    },
    Composition {
        first: 0x3B5,
        second: 0x314,
        result: 0x1F11,
    },
    Composition {
        first: 0x3B7,
        second: 0x300,
        result: 0x1F74,
    },
    Composition {
        first: 0x3B7,
        second: 0x301,
        result: 0x3AE,
    },
    Composition {
        first: 0x3B7,
        second: 0x313,
        result: 0x1F20,
    },
    Composition {
        first: 0x3B7,
        second: 0x314,
        result: 0x1F21,
    },
    Composition {
        first: 0x3B7,
        second: 0x342,
        result: 0x1FC6,
    },
    Composition {
        first: 0x3B7,
        second: 0x345,
        result: 0x1FC3,
    },
    Composition {
        first: 0x3B9,
        second: 0x300,
        result: 0x1F76,
    },
    Composition {
        first: 0x3B9,
        second: 0x301,
        result: 0x3AF,
    },
    Composition {
        first: 0x3B9,
        second: 0x304,
        result: 0x1FD1,
    },
    Composition {
        first: 0x3B9,
        second: 0x306,
        result: 0x1FD0,
    },
    Composition {
        first: 0x3B9,
        second: 0x308,
        result: 0x3CA,
    },
    Composition {
        first: 0x3B9,
        second: 0x313,
        result: 0x1F30,
    },
    Composition {
        first: 0x3B9,
        second: 0x314,
        result: 0x1F31,
    },
    Composition {
        first: 0x3B9,
        second: 0x342,
        result: 0x1FD6,
    },
    Composition {
        first: 0x3BF,
        second: 0x300,
        result: 0x1F78,
    },
    Composition {
        first: 0x3BF,
        second: 0x301,
        result: 0x3CC,
    },
    Composition {
        first: 0x3BF,
        second: 0x313,
        result: 0x1F40,
    },
    Composition {
        first: 0x3BF,
        second: 0x314,
        result: 0x1F41,
    },
    Composition {
        first: 0x3C1,
        second: 0x313,
        result: 0x1FE4,
    },
    Composition {
        first: 0x3C1,
        second: 0x314,
        result: 0x1FE5,
    },
    Composition {
        first: 0x3C5,
        second: 0x300,
        result: 0x1F7A,
    },
    Composition {
        first: 0x3C5,
        second: 0x301,
        result: 0x3CD,
    },
    Composition {
        first: 0x3C5,
        second: 0x304,
        result: 0x1FE1,
    },
    Composition {
        first: 0x3C5,
        second: 0x306,
        result: 0x1FE0,
    },
    Composition {
        first: 0x3C5,
        second: 0x308,
        result: 0x3CB,
    },
    Composition {
        first: 0x3C5,
        second: 0x313,
        result: 0x1F50,
    },
    Composition {
        first: 0x3C5,
        second: 0x314,
        result: 0x1F51,
    },
    Composition {
        first: 0x3C5,
        second: 0x342,
        result: 0x1FE6,
    },
    Composition {
        first: 0x3C9,
        second: 0x300,
        result: 0x1F7C,
    },
    Composition {
        first: 0x3C9,
        second: 0x301,
        result: 0x3CE,
    },
    Composition {
        first: 0x3C9,
        second: 0x313,
        result: 0x1F60,
    },
    Composition {
        first: 0x3C9,
        second: 0x314,
        result: 0x1F61,
    },
    Composition {
        first: 0x3C9,
        second: 0x342,
        result: 0x1FF6,
    },
    Composition {
        first: 0x3C9,
        second: 0x345,
        result: 0x1FF3,
    },
    Composition {
        first: 0x3CA,
        second: 0x300,
        result: 0x1FD2,
    },
    Composition {
        first: 0x3CA,
        second: 0x301,
        result: 0x390,
    },
    Composition {
        first: 0x3CA,
        second: 0x342,
        result: 0x1FD7,
    },
    Composition {
        first: 0x3CB,
        second: 0x300,
        result: 0x1FE2,
    },
    Composition {
        first: 0x3CB,
        second: 0x301,
        result: 0x3B0,
    },
    Composition {
        first: 0x3CB,
        second: 0x342,
        result: 0x1FE7,
    },
    Composition {
        first: 0x3CE,
        second: 0x345,
        result: 0x1FF4,
    },
    Composition {
        first: 0x3D2,
        second: 0x301,
        result: 0x3D3,
    },
    Composition {
        first: 0x3D2,
        second: 0x308,
        result: 0x3D4,
    },
    Composition {
        first: 0x406,
        second: 0x308,
        result: 0x407,
    },
    Composition {
        first: 0x410,
        second: 0x306,
        result: 0x4D0,
    },
    Composition {
        first: 0x410,
        second: 0x308,
        result: 0x4D2,
    },
    Composition {
        first: 0x413,
        second: 0x301,
        result: 0x403,
    },
    Composition {
        first: 0x415,
        second: 0x300,
        result: 0x400,
    },
    Composition {
        first: 0x415,
        second: 0x306,
        result: 0x4D6,
    },
    Composition {
        first: 0x415,
        second: 0x308,
        result: 0x401,
    },
    Composition {
        first: 0x416,
        second: 0x306,
        result: 0x4C1,
    },
    Composition {
        first: 0x416,
        second: 0x308,
        result: 0x4DC,
    },
    Composition {
        first: 0x417,
        second: 0x308,
        result: 0x4DE,
    },
    Composition {
        first: 0x418,
        second: 0x300,
        result: 0x40D,
    },
    Composition {
        first: 0x418,
        second: 0x304,
        result: 0x4E2,
    },
    Composition {
        first: 0x418,
        second: 0x306,
        result: 0x419,
    },
    Composition {
        first: 0x418,
        second: 0x308,
        result: 0x4E4,
    },
    Composition {
        first: 0x41A,
        second: 0x301,
        result: 0x40C,
    },
    Composition {
        first: 0x41E,
        second: 0x308,
        result: 0x4E6,
    },
    Composition {
        first: 0x423,
        second: 0x304,
        result: 0x4EE,
    },
    Composition {
        first: 0x423,
        second: 0x306,
        result: 0x40E,
    },
    Composition {
        first: 0x423,
        second: 0x308,
        result: 0x4F0,
    },
    Composition {
        first: 0x423,
        second: 0x30B,
        result: 0x4F2,
    },
    Composition {
        first: 0x427,
        second: 0x308,
        result: 0x4F4,
    },
    Composition {
        first: 0x42B,
        second: 0x308,
        result: 0x4F8,
    },
    Composition {
        first: 0x42D,
        second: 0x308,
        result: 0x4EC,
    },
    Composition {
        first: 0x430,
        second: 0x306,
        result: 0x4D1,
    },
    Composition {
        first: 0x430,
        second: 0x308,
        result: 0x4D3,
    },
    Composition {
        first: 0x433,
        second: 0x301,
        result: 0x453,
    },
    Composition {
        first: 0x435,
        second: 0x300,
        result: 0x450,
    },
    Composition {
        first: 0x435,
        second: 0x306,
        result: 0x4D7,
    },
    Composition {
        first: 0x435,
        second: 0x308,
        result: 0x451,
    },
    Composition {
        first: 0x436,
        second: 0x306,
        result: 0x4C2,
    },
    Composition {
        first: 0x436,
        second: 0x308,
        result: 0x4DD,
    },
    Composition {
        first: 0x437,
        second: 0x308,
        result: 0x4DF,
    },
    Composition {
        first: 0x438,
        second: 0x300,
        result: 0x45D,
    },
    Composition {
        first: 0x438,
        second: 0x304,
        result: 0x4E3,
    },
    Composition {
        first: 0x438,
        second: 0x306,
        result: 0x439,
    },
    Composition {
        first: 0x438,
        second: 0x308,
        result: 0x4E5,
    },
    Composition {
        first: 0x43A,
        second: 0x301,
        result: 0x45C,
    },
    Composition {
        first: 0x43E,
        second: 0x308,
        result: 0x4E7,
    },
    Composition {
        first: 0x443,
        second: 0x304,
        result: 0x4EF,
    },
    Composition {
        first: 0x443,
        second: 0x306,
        result: 0x45E,
    },
    Composition {
        first: 0x443,
        second: 0x308,
        result: 0x4F1,
    },
    Composition {
        first: 0x443,
        second: 0x30B,
        result: 0x4F3,
    },
    Composition {
        first: 0x447,
        second: 0x308,
        result: 0x4F5,
    },
    Composition {
        first: 0x44B,
        second: 0x308,
        result: 0x4F9,
    },
    Composition {
        first: 0x44D,
        second: 0x308,
        result: 0x4ED,
    },
    Composition {
        first: 0x456,
        second: 0x308,
        result: 0x457,
    },
    Composition {
        first: 0x474,
        second: 0x30F,
        result: 0x476,
    },
    Composition {
        first: 0x475,
        second: 0x30F,
        result: 0x477,
    },
    Composition {
        first: 0x4D8,
        second: 0x308,
        result: 0x4DA,
    },
    Composition {
        first: 0x4D9,
        second: 0x308,
        result: 0x4DB,
    },
    Composition {
        first: 0x4E8,
        second: 0x308,
        result: 0x4EA,
    },
    Composition {
        first: 0x4E9,
        second: 0x308,
        result: 0x4EB,
    },
    Composition {
        first: 0x627,
        second: 0x653,
        result: 0x622,
    },
    Composition {
        first: 0x627,
        second: 0x654,
        result: 0x623,
    },
    Composition {
        first: 0x627,
        second: 0x655,
        result: 0x625,
    },
    Composition {
        first: 0x648,
        second: 0x654,
        result: 0x624,
    },
    Composition {
        first: 0x64A,
        second: 0x654,
        result: 0x626,
    },
    Composition {
        first: 0x6C1,
        second: 0x654,
        result: 0x6C2,
    },
    Composition {
        first: 0x6D2,
        second: 0x654,
        result: 0x6D3,
    },
    Composition {
        first: 0x6D5,
        second: 0x654,
        result: 0x6C0,
    },
    Composition {
        first: 0x928,
        second: 0x93C,
        result: 0x929,
    },
    Composition {
        first: 0x930,
        second: 0x93C,
        result: 0x931,
    },
    Composition {
        first: 0x933,
        second: 0x93C,
        result: 0x934,
    },
    Composition {
        first: 0x9C7,
        second: 0x9BE,
        result: 0x9CB,
    },
    Composition {
        first: 0x9C7,
        second: 0x9D7,
        result: 0x9CC,
    },
    Composition {
        first: 0xB47,
        second: 0xB3E,
        result: 0xB4B,
    },
    Composition {
        first: 0xB47,
        second: 0xB56,
        result: 0xB48,
    },
    Composition {
        first: 0xB47,
        second: 0xB57,
        result: 0xB4C,
    },
    Composition {
        first: 0xB92,
        second: 0xBD7,
        result: 0xB94,
    },
    Composition {
        first: 0xBC6,
        second: 0xBBE,
        result: 0xBCA,
    },
    Composition {
        first: 0xBC6,
        second: 0xBD7,
        result: 0xBCC,
    },
    Composition {
        first: 0xBC7,
        second: 0xBBE,
        result: 0xBCB,
    },
    Composition {
        first: 0xC46,
        second: 0xC56,
        result: 0xC48,
    },
    Composition {
        first: 0xCBF,
        second: 0xCD5,
        result: 0xCC0,
    },
    Composition {
        first: 0xCC6,
        second: 0xCC2,
        result: 0xCCA,
    },
    Composition {
        first: 0xCC6,
        second: 0xCD5,
        result: 0xCC7,
    },
    Composition {
        first: 0xCC6,
        second: 0xCD6,
        result: 0xCC8,
    },
    Composition {
        first: 0xCCA,
        second: 0xCD5,
        result: 0xCCB,
    },
    Composition {
        first: 0xD46,
        second: 0xD3E,
        result: 0xD4A,
    },
    Composition {
        first: 0xD46,
        second: 0xD57,
        result: 0xD4C,
    },
    Composition {
        first: 0xD47,
        second: 0xD3E,
        result: 0xD4B,
    },
    Composition {
        first: 0xDD9,
        second: 0xDCA,
        result: 0xDDA,
    },
    Composition {
        first: 0xDD9,
        second: 0xDCF,
        result: 0xDDC,
    },
    Composition {
        first: 0xDD9,
        second: 0xDDF,
        result: 0xDDE,
    },
    Composition {
        first: 0xDDC,
        second: 0xDCA,
        result: 0xDDD,
    },
    Composition {
        first: 0x1025,
        second: 0x102E,
        result: 0x1026,
    },
    Composition {
        first: 0x1B05,
        second: 0x1B35,
        result: 0x1B06,
    },
    Composition {
        first: 0x1B07,
        second: 0x1B35,
        result: 0x1B08,
    },
    Composition {
        first: 0x1B09,
        second: 0x1B35,
        result: 0x1B0A,
    },
    Composition {
        first: 0x1B0B,
        second: 0x1B35,
        result: 0x1B0C,
    },
    Composition {
        first: 0x1B0D,
        second: 0x1B35,
        result: 0x1B0E,
    },
    Composition {
        first: 0x1B11,
        second: 0x1B35,
        result: 0x1B12,
    },
    Composition {
        first: 0x1B3A,
        second: 0x1B35,
        result: 0x1B3B,
    },
    Composition {
        first: 0x1B3C,
        second: 0x1B35,
        result: 0x1B3D,
    },
    Composition {
        first: 0x1B3E,
        second: 0x1B35,
        result: 0x1B40,
    },
    Composition {
        first: 0x1B3F,
        second: 0x1B35,
        result: 0x1B41,
    },
    Composition {
        first: 0x1B42,
        second: 0x1B35,
        result: 0x1B43,
    },
    Composition {
        first: 0x1E36,
        second: 0x304,
        result: 0x1E38,
    },
    Composition {
        first: 0x1E37,
        second: 0x304,
        result: 0x1E39,
    },
    Composition {
        first: 0x1E5A,
        second: 0x304,
        result: 0x1E5C,
    },
    Composition {
        first: 0x1E5B,
        second: 0x304,
        result: 0x1E5D,
    },
    Composition {
        first: 0x1E62,
        second: 0x307,
        result: 0x1E68,
    },
    Composition {
        first: 0x1E63,
        second: 0x307,
        result: 0x1E69,
    },
    Composition {
        first: 0x1EA0,
        second: 0x302,
        result: 0x1EAC,
    },
    Composition {
        first: 0x1EA0,
        second: 0x306,
        result: 0x1EB6,
    },
    Composition {
        first: 0x1EA1,
        second: 0x302,
        result: 0x1EAD,
    },
    Composition {
        first: 0x1EA1,
        second: 0x306,
        result: 0x1EB7,
    },
    Composition {
        first: 0x1EB8,
        second: 0x302,
        result: 0x1EC6,
    },
    Composition {
        first: 0x1EB9,
        second: 0x302,
        result: 0x1EC7,
    },
    Composition {
        first: 0x1ECC,
        second: 0x302,
        result: 0x1ED8,
    },
    Composition {
        first: 0x1ECD,
        second: 0x302,
        result: 0x1ED9,
    },
    Composition {
        first: 0x1F00,
        second: 0x300,
        result: 0x1F02,
    },
    Composition {
        first: 0x1F00,
        second: 0x301,
        result: 0x1F04,
    },
    Composition {
        first: 0x1F00,
        second: 0x342,
        result: 0x1F06,
    },
    Composition {
        first: 0x1F00,
        second: 0x345,
        result: 0x1F80,
    },
    Composition {
        first: 0x1F01,
        second: 0x300,
        result: 0x1F03,
    },
    Composition {
        first: 0x1F01,
        second: 0x301,
        result: 0x1F05,
    },
    Composition {
        first: 0x1F01,
        second: 0x342,
        result: 0x1F07,
    },
    Composition {
        first: 0x1F01,
        second: 0x345,
        result: 0x1F81,
    },
    Composition {
        first: 0x1F02,
        second: 0x345,
        result: 0x1F82,
    },
    Composition {
        first: 0x1F03,
        second: 0x345,
        result: 0x1F83,
    },
    Composition {
        first: 0x1F04,
        second: 0x345,
        result: 0x1F84,
    },
    Composition {
        first: 0x1F05,
        second: 0x345,
        result: 0x1F85,
    },
    Composition {
        first: 0x1F06,
        second: 0x345,
        result: 0x1F86,
    },
    Composition {
        first: 0x1F07,
        second: 0x345,
        result: 0x1F87,
    },
    Composition {
        first: 0x1F08,
        second: 0x300,
        result: 0x1F0A,
    },
    Composition {
        first: 0x1F08,
        second: 0x301,
        result: 0x1F0C,
    },
    Composition {
        first: 0x1F08,
        second: 0x342,
        result: 0x1F0E,
    },
    Composition {
        first: 0x1F08,
        second: 0x345,
        result: 0x1F88,
    },
    Composition {
        first: 0x1F09,
        second: 0x300,
        result: 0x1F0B,
    },
    Composition {
        first: 0x1F09,
        second: 0x301,
        result: 0x1F0D,
    },
    Composition {
        first: 0x1F09,
        second: 0x342,
        result: 0x1F0F,
    },
    Composition {
        first: 0x1F09,
        second: 0x345,
        result: 0x1F89,
    },
    Composition {
        first: 0x1F0A,
        second: 0x345,
        result: 0x1F8A,
    },
    Composition {
        first: 0x1F0B,
        second: 0x345,
        result: 0x1F8B,
    },
    Composition {
        first: 0x1F0C,
        second: 0x345,
        result: 0x1F8C,
    },
    Composition {
        first: 0x1F0D,
        second: 0x345,
        result: 0x1F8D,
    },
    Composition {
        first: 0x1F0E,
        second: 0x345,
        result: 0x1F8E,
    },
    Composition {
        first: 0x1F0F,
        second: 0x345,
        result: 0x1F8F,
    },
    Composition {
        first: 0x1F10,
        second: 0x300,
        result: 0x1F12,
    },
    Composition {
        first: 0x1F10,
        second: 0x301,
        result: 0x1F14,
    },
    Composition {
        first: 0x1F11,
        second: 0x300,
        result: 0x1F13,
    },
    Composition {
        first: 0x1F11,
        second: 0x301,
        result: 0x1F15,
    },
    Composition {
        first: 0x1F18,
        second: 0x300,
        result: 0x1F1A,
    },
    Composition {
        first: 0x1F18,
        second: 0x301,
        result: 0x1F1C,
    },
    Composition {
        first: 0x1F19,
        second: 0x300,
        result: 0x1F1B,
    },
    Composition {
        first: 0x1F19,
        second: 0x301,
        result: 0x1F1D,
    },
    Composition {
        first: 0x1F20,
        second: 0x300,
        result: 0x1F22,
    },
    Composition {
        first: 0x1F20,
        second: 0x301,
        result: 0x1F24,
    },
    Composition {
        first: 0x1F20,
        second: 0x342,
        result: 0x1F26,
    },
    Composition {
        first: 0x1F20,
        second: 0x345,
        result: 0x1F90,
    },
    Composition {
        first: 0x1F21,
        second: 0x300,
        result: 0x1F23,
    },
    Composition {
        first: 0x1F21,
        second: 0x301,
        result: 0x1F25,
    },
    Composition {
        first: 0x1F21,
        second: 0x342,
        result: 0x1F27,
    },
    Composition {
        first: 0x1F21,
        second: 0x345,
        result: 0x1F91,
    },
    Composition {
        first: 0x1F22,
        second: 0x345,
        result: 0x1F92,
    },
    Composition {
        first: 0x1F23,
        second: 0x345,
        result: 0x1F93,
    },
    Composition {
        first: 0x1F24,
        second: 0x345,
        result: 0x1F94,
    },
    Composition {
        first: 0x1F25,
        second: 0x345,
        result: 0x1F95,
    },
    Composition {
        first: 0x1F26,
        second: 0x345,
        result: 0x1F96,
    },
    Composition {
        first: 0x1F27,
        second: 0x345,
        result: 0x1F97,
    },
    Composition {
        first: 0x1F28,
        second: 0x300,
        result: 0x1F2A,
    },
    Composition {
        first: 0x1F28,
        second: 0x301,
        result: 0x1F2C,
    },
    Composition {
        first: 0x1F28,
        second: 0x342,
        result: 0x1F2E,
    },
    Composition {
        first: 0x1F28,
        second: 0x345,
        result: 0x1F98,
    },
    Composition {
        first: 0x1F29,
        second: 0x300,
        result: 0x1F2B,
    },
    Composition {
        first: 0x1F29,
        second: 0x301,
        result: 0x1F2D,
    },
    Composition {
        first: 0x1F29,
        second: 0x342,
        result: 0x1F2F,
    },
    Composition {
        first: 0x1F29,
        second: 0x345,
        result: 0x1F99,
    },
    Composition {
        first: 0x1F2A,
        second: 0x345,
        result: 0x1F9A,
    },
    Composition {
        first: 0x1F2B,
        second: 0x345,
        result: 0x1F9B,
    },
    Composition {
        first: 0x1F2C,
        second: 0x345,
        result: 0x1F9C,
    },
    Composition {
        first: 0x1F2D,
        second: 0x345,
        result: 0x1F9D,
    },
    Composition {
        first: 0x1F2E,
        second: 0x345,
        result: 0x1F9E,
    },
    Composition {
        first: 0x1F2F,
        second: 0x345,
        result: 0x1F9F,
    },
    Composition {
        first: 0x1F30,
        second: 0x300,
        result: 0x1F32,
    },
    Composition {
        first: 0x1F30,
        second: 0x301,
        result: 0x1F34,
    },
    Composition {
        first: 0x1F30,
        second: 0x342,
        result: 0x1F36,
    },
    Composition {
        first: 0x1F31,
        second: 0x300,
        result: 0x1F33,
    },
    Composition {
        first: 0x1F31,
        second: 0x301,
        result: 0x1F35,
    },
    Composition {
        first: 0x1F31,
        second: 0x342,
        result: 0x1F37,
    },
    Composition {
        first: 0x1F38,
        second: 0x300,
        result: 0x1F3A,
    },
    Composition {
        first: 0x1F38,
        second: 0x301,
        result: 0x1F3C,
    },
    Composition {
        first: 0x1F38,
        second: 0x342,
        result: 0x1F3E,
    },
    Composition {
        first: 0x1F39,
        second: 0x300,
        result: 0x1F3B,
    },
    Composition {
        first: 0x1F39,
        second: 0x301,
        result: 0x1F3D,
    },
    Composition {
        first: 0x1F39,
        second: 0x342,
        result: 0x1F3F,
    },
    Composition {
        first: 0x1F40,
        second: 0x300,
        result: 0x1F42,
    },
    Composition {
        first: 0x1F40,
        second: 0x301,
        result: 0x1F44,
    },
    Composition {
        first: 0x1F41,
        second: 0x300,
        result: 0x1F43,
    },
    Composition {
        first: 0x1F41,
        second: 0x301,
        result: 0x1F45,
    },
    Composition {
        first: 0x1F48,
        second: 0x300,
        result: 0x1F4A,
    },
    Composition {
        first: 0x1F48,
        second: 0x301,
        result: 0x1F4C,
    },
    Composition {
        first: 0x1F49,
        second: 0x300,
        result: 0x1F4B,
    },
    Composition {
        first: 0x1F49,
        second: 0x301,
        result: 0x1F4D,
    },
    Composition {
        first: 0x1F50,
        second: 0x300,
        result: 0x1F52,
    },
    Composition {
        first: 0x1F50,
        second: 0x301,
        result: 0x1F54,
    },
    Composition {
        first: 0x1F50,
        second: 0x342,
        result: 0x1F56,
    },
    Composition {
        first: 0x1F51,
        second: 0x300,
        result: 0x1F53,
    },
    Composition {
        first: 0x1F51,
        second: 0x301,
        result: 0x1F55,
    },
    Composition {
        first: 0x1F51,
        second: 0x342,
        result: 0x1F57,
    },
    Composition {
        first: 0x1F59,
        second: 0x300,
        result: 0x1F5B,
    },
    Composition {
        first: 0x1F59,
        second: 0x301,
        result: 0x1F5D,
    },
    Composition {
        first: 0x1F59,
        second: 0x342,
        result: 0x1F5F,
    },
    Composition {
        first: 0x1F60,
        second: 0x300,
        result: 0x1F62,
    },
    Composition {
        first: 0x1F60,
        second: 0x301,
        result: 0x1F64,
    },
    Composition {
        first: 0x1F60,
        second: 0x342,
        result: 0x1F66,
    },
    Composition {
        first: 0x1F60,
        second: 0x345,
        result: 0x1FA0,
    },
    Composition {
        first: 0x1F61,
        second: 0x300,
        result: 0x1F63,
    },
    Composition {
        first: 0x1F61,
        second: 0x301,
        result: 0x1F65,
    },
    Composition {
        first: 0x1F61,
        second: 0x342,
        result: 0x1F67,
    },
    Composition {
        first: 0x1F61,
        second: 0x345,
        result: 0x1FA1,
    },
    Composition {
        first: 0x1F62,
        second: 0x345,
        result: 0x1FA2,
    },
    Composition {
        first: 0x1F63,
        second: 0x345,
        result: 0x1FA3,
    },
    Composition {
        first: 0x1F64,
        second: 0x345,
        result: 0x1FA4,
    },
    Composition {
        first: 0x1F65,
        second: 0x345,
        result: 0x1FA5,
    },
    Composition {
        first: 0x1F66,
        second: 0x345,
        result: 0x1FA6,
    },
    Composition {
        first: 0x1F67,
        second: 0x345,
        result: 0x1FA7,
    },
    Composition {
        first: 0x1F68,
        second: 0x300,
        result: 0x1F6A,
    },
    Composition {
        first: 0x1F68,
        second: 0x301,
        result: 0x1F6C,
    },
    Composition {
        first: 0x1F68,
        second: 0x342,
        result: 0x1F6E,
    },
    Composition {
        first: 0x1F68,
        second: 0x345,
        result: 0x1FA8,
    },
    Composition {
        first: 0x1F69,
        second: 0x300,
        result: 0x1F6B,
    },
    Composition {
        first: 0x1F69,
        second: 0x301,
        result: 0x1F6D,
    },
    Composition {
        first: 0x1F69,
        second: 0x342,
        result: 0x1F6F,
    },
    Composition {
        first: 0x1F69,
        second: 0x345,
        result: 0x1FA9,
    },
    Composition {
        first: 0x1F6A,
        second: 0x345,
        result: 0x1FAA,
    },
    Composition {
        first: 0x1F6B,
        second: 0x345,
        result: 0x1FAB,
    },
    Composition {
        first: 0x1F6C,
        second: 0x345,
        result: 0x1FAC,
    },
    Composition {
        first: 0x1F6D,
        second: 0x345,
        result: 0x1FAD,
    },
    Composition {
        first: 0x1F6E,
        second: 0x345,
        result: 0x1FAE,
    },
    Composition {
        first: 0x1F6F,
        second: 0x345,
        result: 0x1FAF,
    },
    Composition {
        first: 0x1F70,
        second: 0x345,
        result: 0x1FB2,
    },
    Composition {
        first: 0x1F74,
        second: 0x345,
        result: 0x1FC2,
    },
    Composition {
        first: 0x1F7C,
        second: 0x345,
        result: 0x1FF2,
    },
    Composition {
        first: 0x1FB6,
        second: 0x345,
        result: 0x1FB7,
    },
    Composition {
        first: 0x1FBF,
        second: 0x300,
        result: 0x1FCD,
    },
    Composition {
        first: 0x1FBF,
        second: 0x301,
        result: 0x1FCE,
    },
    Composition {
        first: 0x1FBF,
        second: 0x342,
        result: 0x1FCF,
    },
    Composition {
        first: 0x1FC6,
        second: 0x345,
        result: 0x1FC7,
    },
    Composition {
        first: 0x1FF6,
        second: 0x345,
        result: 0x1FF7,
    },
    Composition {
        first: 0x1FFE,
        second: 0x300,
        result: 0x1FDD,
    },
    Composition {
        first: 0x1FFE,
        second: 0x301,
        result: 0x1FDE,
    },
    Composition {
        first: 0x1FFE,
        second: 0x342,
        result: 0x1FDF,
    },
    Composition {
        first: 0x2190,
        second: 0x338,
        result: 0x219A,
    },
    Composition {
        first: 0x2192,
        second: 0x338,
        result: 0x219B,
    },
    Composition {
        first: 0x2194,
        second: 0x338,
        result: 0x21AE,
    },
    Composition {
        first: 0x21D0,
        second: 0x338,
        result: 0x21CD,
    },
    Composition {
        first: 0x21D2,
        second: 0x338,
        result: 0x21CF,
    },
    Composition {
        first: 0x21D4,
        second: 0x338,
        result: 0x21CE,
    },
    Composition {
        first: 0x2203,
        second: 0x338,
        result: 0x2204,
    },
    Composition {
        first: 0x2208,
        second: 0x338,
        result: 0x2209,
    },
    Composition {
        first: 0x220B,
        second: 0x338,
        result: 0x220C,
    },
    Composition {
        first: 0x2223,
        second: 0x338,
        result: 0x2224,
    },
    Composition {
        first: 0x2225,
        second: 0x338,
        result: 0x2226,
    },
    Composition {
        first: 0x223C,
        second: 0x338,
        result: 0x2241,
    },
    Composition {
        first: 0x2243,
        second: 0x338,
        result: 0x2244,
    },
    Composition {
        first: 0x2245,
        second: 0x338,
        result: 0x2247,
    },
    Composition {
        first: 0x2248,
        second: 0x338,
        result: 0x2249,
    },
    Composition {
        first: 0x224D,
        second: 0x338,
        result: 0x226D,
    },
    Composition {
        first: 0x2261,
        second: 0x338,
        result: 0x2262,
    },
    Composition {
        first: 0x2264,
        second: 0x338,
        result: 0x2270,
    },
    Composition {
        first: 0x2265,
        second: 0x338,
        result: 0x2271,
    },
    Composition {
        first: 0x2272,
        second: 0x338,
        result: 0x2274,
    },
    Composition {
        first: 0x2273,
        second: 0x338,
        result: 0x2275,
    },
    Composition {
        first: 0x2276,
        second: 0x338,
        result: 0x2278,
    },
    Composition {
        first: 0x2277,
        second: 0x338,
        result: 0x2279,
    },
    Composition {
        first: 0x227A,
        second: 0x338,
        result: 0x2280,
    },
    Composition {
        first: 0x227B,
        second: 0x338,
        result: 0x2281,
    },
    Composition {
        first: 0x227C,
        second: 0x338,
        result: 0x22E0,
    },
    Composition {
        first: 0x227D,
        second: 0x338,
        result: 0x22E1,
    },
    Composition {
        first: 0x2282,
        second: 0x338,
        result: 0x2284,
    },
    Composition {
        first: 0x2283,
        second: 0x338,
        result: 0x2285,
    },
    Composition {
        first: 0x2286,
        second: 0x338,
        result: 0x2288,
    },
    Composition {
        first: 0x2287,
        second: 0x338,
        result: 0x2289,
    },
    Composition {
        first: 0x2291,
        second: 0x338,
        result: 0x22E2,
    },
    Composition {
        first: 0x2292,
        second: 0x338,
        result: 0x22E3,
    },
    Composition {
        first: 0x22A2,
        second: 0x338,
        result: 0x22AC,
    },
    Composition {
        first: 0x22A8,
        second: 0x338,
        result: 0x22AD,
    },
    Composition {
        first: 0x22A9,
        second: 0x338,
        result: 0x22AE,
    },
    Composition {
        first: 0x22AB,
        second: 0x338,
        result: 0x22AF,
    },
    Composition {
        first: 0x22B2,
        second: 0x338,
        result: 0x22EA,
    },
    Composition {
        first: 0x22B3,
        second: 0x338,
        result: 0x22EB,
    },
    Composition {
        first: 0x22B4,
        second: 0x338,
        result: 0x22EC,
    },
    Composition {
        first: 0x22B5,
        second: 0x338,
        result: 0x22ED,
    },
    Composition {
        first: 0x3046,
        second: 0x3099,
        result: 0x3094,
    },
    Composition {
        first: 0x304B,
        second: 0x3099,
        result: 0x304C,
    },
    Composition {
        first: 0x304D,
        second: 0x3099,
        result: 0x304E,
    },
    Composition {
        first: 0x304F,
        second: 0x3099,
        result: 0x3050,
    },
    Composition {
        first: 0x3051,
        second: 0x3099,
        result: 0x3052,
    },
    Composition {
        first: 0x3053,
        second: 0x3099,
        result: 0x3054,
    },
    Composition {
        first: 0x3055,
        second: 0x3099,
        result: 0x3056,
    },
    Composition {
        first: 0x3057,
        second: 0x3099,
        result: 0x3058,
    },
    Composition {
        first: 0x3059,
        second: 0x3099,
        result: 0x305A,
    },
    Composition {
        first: 0x305B,
        second: 0x3099,
        result: 0x305C,
    },
    Composition {
        first: 0x305D,
        second: 0x3099,
        result: 0x305E,
    },
    Composition {
        first: 0x305F,
        second: 0x3099,
        result: 0x3060,
    },
    Composition {
        first: 0x3061,
        second: 0x3099,
        result: 0x3062,
    },
    Composition {
        first: 0x3064,
        second: 0x3099,
        result: 0x3065,
    },
    Composition {
        first: 0x3066,
        second: 0x3099,
        result: 0x3067,
    },
    Composition {
        first: 0x3068,
        second: 0x3099,
        result: 0x3069,
    },
    Composition {
        first: 0x306F,
        second: 0x3099,
        result: 0x3070,
    },
    Composition {
        first: 0x306F,
        second: 0x309A,
        result: 0x3071,
    },
    Composition {
        first: 0x3072,
        second: 0x3099,
        result: 0x3073,
    },
    Composition {
        first: 0x3072,
        second: 0x309A,
        result: 0x3074,
    },
    Composition {
        first: 0x3075,
        second: 0x3099,
        result: 0x3076,
    },
    Composition {
        first: 0x3075,
        second: 0x309A,
        result: 0x3077,
    },
    Composition {
        first: 0x3078,
        second: 0x3099,
        result: 0x3079,
    },
    Composition {
        first: 0x3078,
        second: 0x309A,
        result: 0x307A,
    },
    Composition {
        first: 0x307B,
        second: 0x3099,
        result: 0x307C,
    },
    Composition {
        first: 0x307B,
        second: 0x309A,
        result: 0x307D,
    },
    Composition {
        first: 0x309D,
        second: 0x3099,
        result: 0x309E,
    },
    Composition {
        first: 0x30A6,
        second: 0x3099,
        result: 0x30F4,
    },
    Composition {
        first: 0x30AB,
        second: 0x3099,
        result: 0x30AC,
    },
    Composition {
        first: 0x30AD,
        second: 0x3099,
        result: 0x30AE,
    },
    Composition {
        first: 0x30AF,
        second: 0x3099,
        result: 0x30B0,
    },
    Composition {
        first: 0x30B1,
        second: 0x3099,
        result: 0x30B2,
    },
    Composition {
        first: 0x30B3,
        second: 0x3099,
        result: 0x30B4,
    },
    Composition {
        first: 0x30B5,
        second: 0x3099,
        result: 0x30B6,
    },
    Composition {
        first: 0x30B7,
        second: 0x3099,
        result: 0x30B8,
    },
    Composition {
        first: 0x30B9,
        second: 0x3099,
        result: 0x30BA,
    },
    Composition {
        first: 0x30BB,
        second: 0x3099,
        result: 0x30BC,
    },
    Composition {
        first: 0x30BD,
        second: 0x3099,
        result: 0x30BE,
    },
    Composition {
        first: 0x30BF,
        second: 0x3099,
        result: 0x30C0,
    },
    Composition {
        first: 0x30C1,
        second: 0x3099,
        result: 0x30C2,
    },
    Composition {
        first: 0x30C4,
        second: 0x3099,
        result: 0x30C5,
    },
    Composition {
        first: 0x30C6,
        second: 0x3099,
        result: 0x30C7,
    },
    Composition {
        first: 0x30C8,
        second: 0x3099,
        result: 0x30C9,
    },
    Composition {
        first: 0x30CF,
        second: 0x3099,
        result: 0x30D0,
    },
    Composition {
        first: 0x30CF,
        second: 0x309A,
        result: 0x30D1,
    },
    Composition {
        first: 0x30D2,
        second: 0x3099,
        result: 0x30D3,
    },
    Composition {
        first: 0x30D2,
        second: 0x309A,
        result: 0x30D4,
    },
    Composition {
        first: 0x30D5,
        second: 0x3099,
        result: 0x30D6,
    },
    Composition {
        first: 0x30D5,
        second: 0x309A,
        result: 0x30D7,
    },
    Composition {
        first: 0x30D8,
        second: 0x3099,
        result: 0x30D9,
    },
    Composition {
        first: 0x30D8,
        second: 0x309A,
        result: 0x30DA,
    },
    Composition {
        first: 0x30DB,
        second: 0x3099,
        result: 0x30DC,
    },
    Composition {
        first: 0x30DB,
        second: 0x309A,
        result: 0x30DD,
    },
    Composition {
        first: 0x30EF,
        second: 0x3099,
        result: 0x30F7,
    },
    Composition {
        first: 0x30F0,
        second: 0x3099,
        result: 0x30F8,
    },
    Composition {
        first: 0x30F1,
        second: 0x3099,
        result: 0x30F9,
    },
    Composition {
        first: 0x30F2,
        second: 0x3099,
        result: 0x30FA,
    },
    Composition {
        first: 0x30FD,
        second: 0x3099,
        result: 0x30FE,
    },
    Composition {
        first: 0x105D2,
        second: 0x307,
        result: 0x105C9,
    },
    Composition {
        first: 0x105DA,
        second: 0x307,
        result: 0x105E4,
    },
    Composition {
        first: 0x11099,
        second: 0x110BA,
        result: 0x1109A,
    },
    Composition {
        first: 0x1109B,
        second: 0x110BA,
        result: 0x1109C,
    },
    Composition {
        first: 0x110A5,
        second: 0x110BA,
        result: 0x110AB,
    },
    Composition {
        first: 0x11131,
        second: 0x11127,
        result: 0x1112E,
    },
    Composition {
        first: 0x11132,
        second: 0x11127,
        result: 0x1112F,
    },
    Composition {
        first: 0x11347,
        second: 0x1133E,
        result: 0x1134B,
    },
    Composition {
        first: 0x11347,
        second: 0x11357,
        result: 0x1134C,
    },
    Composition {
        first: 0x11382,
        second: 0x113C9,
        result: 0x11383,
    },
    Composition {
        first: 0x11384,
        second: 0x113BB,
        result: 0x11385,
    },
    Composition {
        first: 0x1138B,
        second: 0x113C2,
        result: 0x1138E,
    },
    Composition {
        first: 0x11390,
        second: 0x113C9,
        result: 0x11391,
    },
    Composition {
        first: 0x113C2,
        second: 0x113B8,
        result: 0x113C7,
    },
    Composition {
        first: 0x113C2,
        second: 0x113C2,
        result: 0x113C5,
    },
    Composition {
        first: 0x113C2,
        second: 0x113C9,
        result: 0x113C8,
    },
    Composition {
        first: 0x114B9,
        second: 0x114B0,
        result: 0x114BC,
    },
    Composition {
        first: 0x114B9,
        second: 0x114BA,
        result: 0x114BB,
    },
    Composition {
        first: 0x114B9,
        second: 0x114BD,
        result: 0x114BE,
    },
    Composition {
        first: 0x115B8,
        second: 0x115AF,
        result: 0x115BA,
    },
    Composition {
        first: 0x115B9,
        second: 0x115AF,
        result: 0x115BB,
    },
    Composition {
        first: 0x11935,
        second: 0x11930,
        result: 0x11938,
    },
    Composition {
        first: 0x1611E,
        second: 0x1611E,
        result: 0x16121,
    },
    Composition {
        first: 0x1611E,
        second: 0x1611F,
        result: 0x16123,
    },
    Composition {
        first: 0x1611E,
        second: 0x16120,
        result: 0x16125,
    },
    Composition {
        first: 0x1611E,
        second: 0x16129,
        result: 0x16122,
    },
    Composition {
        first: 0x16121,
        second: 0x1611F,
        result: 0x16126,
    },
    Composition {
        first: 0x16121,
        second: 0x16120,
        result: 0x16128,
    },
    Composition {
        first: 0x16122,
        second: 0x1611F,
        result: 0x16127,
    },
    Composition {
        first: 0x16129,
        second: 0x1611F,
        result: 0x16124,
    },
    Composition {
        first: 0x16D63,
        second: 0x16D67,
        result: 0x16D69,
    },
    Composition {
        first: 0x16D67,
        second: 0x16D67,
        result: 0x16D68,
    },
    Composition {
        first: 0x16D69,
        second: 0x16D67,
        result: 0x16D6A,
    },
];
