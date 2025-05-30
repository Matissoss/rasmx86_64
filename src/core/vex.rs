// rasmx86_64 - src/core/vex.rs
// ----------------------------
// made by matissoss
// licensed under MPL 2.0

use crate::core::api;

use crate::shr::{
    ast::{IVariant, Instruction, Operand},
    ins::Mnemonic as Ins,
    segment::Segment,
};

const TWO_BYTE_PFX: u8 = 0xC5;
const THREE_BYTE_PFX: u8 = 0xC4;

pub fn vex(ins: &Instruction, ctx: &api::GenAPI) -> Option<Vec<u8>> {
    use api::OpOrd::*;
    let (modrm_reg, modrm_rm, vex_opr) = match ctx.get_ord()[0..3] {
        [VEX_VVVV, MODRM_REG, MODRM_RM] => (ins.src(), ins.src2(), ins.dst()),
        [VEX_VVVV, MODRM_RM, MODRM_REG] => (ins.src2(), ins.src(), ins.dst()),
        [MODRM_REG, VEX_VVVV, MODRM_RM] => (ins.dst(), ins.src2(), ins.src()),
        [MODRM_RM, VEX_VVVV, MODRM_REG] => (ins.src2(), ins.dst(), ins.src()),
        [MODRM_RM, MODRM_REG, VEX_VVVV] => (ins.src(), ins.dst(), ins.src2()),
        [MODRM_REG, MODRM_RM, VEX_VVVV] => (ins.dst(), ins.src(), ins.src2()),
        [VEX_VVVV, MODRM_REG, _] => (ins.src(), None, ins.dst()),
        [VEX_VVVV, MODRM_RM, _] => (None, ins.src(), ins.dst()),
        _ => (ins.src2(), ins.dst(), ins.src()),
    };
    let vvvv = gen_vex4v(vex_opr);
    let pp = ctx.get_pp().unwrap();
    let map_select = ctx.get_map_select().unwrap();
    let vex_we = ctx.get_vex_we().unwrap();

    let tmp = ins.which_variant() == IVariant::YMM;
    let vlength = {
        if let Some(mg) = ctx.get_vex_vlength() {
            if let Some(ob) = mg.get() {
                ob as u8
            } else {
                tmp as u8
            }
        } else {
            tmp as u8
        }
    };

    let vex_b = needs_vex3(modrm_rm);
    let vex_r = needs_vex3(modrm_reg).0;

    if (vex_b.0 || vex_b.1) || (map_select == 0b00011 || map_select == 0b00010) || vex_we {
        Some(vec![
            THREE_BYTE_PFX,
            (((!vex_r) as u8) << 7
                | andn(vex_b.1 as u8, 0b0000_0001) << 6
                | (andn(vex_b.0 as u8, 0b0000_0001) << 5 | map_select)),
            ((vex_we as u8) << 7 | vvvv << 3 | vlength << 2 | pp),
        ])
    } else {
        Some(vec![
            TWO_BYTE_PFX,
            ((((!vex_r) as u8) << 7) | vvvv << 3 | vlength << 2 | pp),
        ])
    }
}

#[allow(clippy::too_many_arguments)]
pub fn gen_vex_norm(
    ins: &Instruction,
    pp: u8,
    map_select: u8,
    modrm_reg_is_dst: bool,
    vex_we: bool,
    dst: Option<&Operand>,
    src: Option<&Operand>,
    src2: Option<&Operand>,
) -> Option<Vec<u8>> {
    let ssrc = if let Some(src2) = src2 {
        if let Operand::Imm(_) = src2 {
            0b1111
        } else {
            gen_vex4v(src)
        }
    } else {
        0b1111
    };

    let pp = match pp {
        0x66 => 0b01,
        0xF3 => 0b10,
        0xF2 => 0b11,
        _ => pp,
    };
    let vlength =
        (ins.which_variant() == IVariant::YMM || matches!(ins.mnem, Ins::VEXTRACTF128)) as u8;

    let map_select = match map_select {
        0x0F => 0b00001,
        0x38 => 0b00010,
        0x3A => 0b00011,
        _ => map_select,
    };

    let nvex_dst = needs_vex3(dst);
    let nvex_src = needs_vex3(src);
    let nvex_ssrc = needs_vex3(src2);

    let (vexr, vexb) = if src2.is_some() {
        if modrm_reg_is_dst {
            (andn((nvex_dst.0 || nvex_dst.1) as u8, 1), nvex_ssrc)
        } else {
            (andn((nvex_ssrc.0 || nvex_ssrc.1) as u8, 1), nvex_dst)
        }
    } else {
        if modrm_reg_is_dst {
            (andn((nvex_dst.0 || nvex_dst.1) as u8, 1), nvex_src)
        } else {
            (andn((nvex_src.0 || nvex_src.1) as u8, 1), nvex_dst)
        }
    };

    if vexb.0
        || vexb.1
        || ((map_select == 0b00011 || map_select == 0b00010) && !matches!(ins.mnem, Ins::VPMAXUB))
        || vex_we
    {
        Some(vec![
            THREE_BYTE_PFX,
            ((vexr) << 7
                | andn(vexb.1 as u8, 0b0000_0001) << 6
                | (andn(vexb.0 as u8, 0b0000_0001) << 5 | map_select)),
            ((vex_we as u8) << 7 | ssrc << 3 | vlength << 2 | pp),
        ])
    } else {
        Some(vec![
            TWO_BYTE_PFX,
            (((vexr) << 7) | ssrc << 3 | vlength << 2 | pp),
        ])
    }
}

pub fn gen_vex(
    ins: &Instruction,
    pp: u8,
    map_select: u8,
    modrm_reg_is_dst: bool,
    vex_we: bool,
) -> Option<Vec<u8>> {
    let dst = ins.dst();
    let src = ins.src();
    let src2 = ins.src2();
    let ssrc = if let Some(src2) = src2 {
        if let Operand::Imm(_) = src2 {
            0b1111
        } else {
            gen_vex4v(src)
        }
    } else {
        0b1111
    };

    let pp = match pp {
        0x66 => 0b01,
        0xF3 => 0b10,
        0xF2 => 0b11,
        _ => pp,
    };
    let vlength =
        (ins.which_variant() == IVariant::YMM || matches!(ins.mnem, Ins::VEXTRACTF128)) as u8;

    let map_select = match map_select {
        0x0F => 0b00001,
        0x38 => 0b00010,
        0x3A => 0b00011,
        _ => map_select,
    };

    let nvex_dst = needs_vex3(dst);
    let nvex_src = needs_vex3(src);
    let nvex_ssrc = needs_vex3(src2);

    let (vexr, vexb) = if src2.is_some() {
        if modrm_reg_is_dst {
            (andn((nvex_dst.0 || nvex_dst.1) as u8, 1), nvex_ssrc)
        } else {
            (andn((nvex_ssrc.0 || nvex_ssrc.1) as u8, 1), nvex_dst)
        }
    } else {
        if modrm_reg_is_dst {
            (andn((nvex_dst.0 || nvex_dst.1) as u8, 1), nvex_src)
        } else {
            (andn((nvex_src.0 || nvex_src.1) as u8, 1), nvex_dst)
        }
    };

    if vexb.0
        || vexb.1
        || ((map_select == 0b00011 || map_select == 0b00010) && !matches!(ins.mnem, Ins::VPMAXUB))
        || vex_we
    {
        Some(vec![
            THREE_BYTE_PFX,
            ((vexr) << 7
                | andn(vexb.1 as u8, 0b0000_0001) << 6
                | (andn(vexb.0 as u8, 0b0000_0001) << 5 | map_select)),
            ((vex_we as u8) << 7 | ssrc << 3 | vlength << 2 | pp),
        ])
    } else {
        Some(vec![
            TWO_BYTE_PFX,
            (((vexr) << 7) | ssrc << 3 | vlength << 2 | pp),
        ])
    }
}

fn needs_vex3(op: Option<&Operand>) -> (bool, bool) {
    if let Some(op) = op {
        match op {
            Operand::Reg(r) => {
                if r.needs_rex() {
                    return (true, false);
                }
            }
            Operand::Mem(m) => {
                let rr = m.needs_rex();
                if rr.0 || rr.1 {
                    return rr;
                }
            }
            _ => {}
        }
    }
    (false, false)
}

const fn andn(num: u8, bits: u8) -> u8 {
    !num & bits
}

// VEX.vvvv field
#[allow(clippy::collapsible_match)]
fn gen_vex4v(op: Option<&Operand>) -> u8 {
    if let Some(o) = op {
        match o {
            Operand::Reg(r) => andn((r.needs_rex() as u8) << 3 | r.to_byte(), 0b0000_1111),
            _ => 0b1111,
        }
    } else {
        0b1111
    }
}

// copied from src/core/modrm.rs:gen_modrm
pub fn vex_modrm(ins: &Instruction, reg: Option<u8>, rm: Option<u8>, modrm_reg_is_dst: bool) -> u8 {
    let dst = ins.dst();
    let src2 = ins.src2();
    let mod_ = {
        if let Some(sibidx) = ins.get_sib_idx() {
            match ins.oprs.get(sibidx).unwrap() {
                Operand::Mem(m)
                | Operand::Segment(Segment {
                    address: m,
                    segment: _,
                }) => {
                    if m.is_sib() {
                        if let Some((_, sz)) = m.offset_x86() {
                            if sz == 1 {
                                0b01
                            } else {
                                0b10
                            }
                        } else {
                            0b00
                        }
                    } else {
                        if let Some((_, sz)) = m.offset_x86() {
                            if sz == 1 {
                                0b01
                            } else {
                                0b10
                            }
                        } else {
                            0b00
                        }
                    }
                }
                _ => 0b11,
            }
        } else {
            match (dst, src2) {
                (
                    Some(
                        Operand::Mem(m)
                        | Operand::Segment(Segment {
                            address: m,
                            segment: _,
                        }),
                    ),
                    _,
                )
                | (
                    _,
                    Some(
                        Operand::Mem(m)
                        | Operand::Segment(Segment {
                            address: m,
                            segment: _,
                        }),
                    ),
                ) => {
                    if m.is_sib() {
                        if let Some((_, sz)) = m.offset_x86() {
                            if sz == 1 {
                                0b01
                            } else {
                                0b10
                            }
                        } else {
                            0b00
                        }
                    } else {
                        if let Some((_, sz)) = m.offset_x86() {
                            if sz == 1 {
                                0b01
                            } else {
                                0b10
                            }
                        } else {
                            0b00
                        }
                    }
                }
                _ => 0b11,
            }
        }
    };
    let mut modrm_reg_is_dst = modrm_reg_is_dst;

    let ssrc = src2;

    if matches!(ins.mnem, Ins::VINSERTF128) {
        modrm_reg_is_dst = true;
    }

    let reg = if let Some(reg) = reg {
        reg
    } else {
        if modrm_reg_is_dst {
            gen_rmreg(dst)
        } else {
            if let Some(Operand::Mem(_) | Operand::Segment(_)) = ssrc {
                modrm_reg_is_dst = true;
                gen_rmreg(dst)
            } else {
                gen_rmreg(ssrc)
            }
        }
    };
    let rm = {
        if ins.uses_sib() {
            0b100
        } else {
            if let Some(rm) = rm {
                rm
            } else {
                if modrm_reg_is_dst {
                    gen_rmreg(ssrc)
                } else {
                    gen_rmreg(dst)
                }
            }
        }
    };

    (mod_ << 6) + (reg << 3) + rm
}

pub fn vex_modrm_norm(
    ins: &Instruction,
    reg: Option<u8>,
    rm: Option<u8>,
    modrm_reg_is_dst: bool,
    dst: Option<&Operand>,
    src2: Option<&Operand>,
) -> u8 {
    let mod_ = {
        if let Some(sibidx) = ins.get_sib_idx() {
            match ins.oprs.get(sibidx).unwrap() {
                Operand::Mem(m)
                | Operand::Segment(Segment {
                    address: m,
                    segment: _,
                }) => {
                    if m.is_sib() {
                        if let Some((_, sz)) = m.offset_x86() {
                            if sz == 1 {
                                0b01
                            } else {
                                0b10
                            }
                        } else {
                            0b00
                        }
                    } else {
                        if let Some((_, sz)) = m.offset_x86() {
                            if sz == 1 {
                                0b01
                            } else {
                                0b10
                            }
                        } else {
                            0b00
                        }
                    }
                }
                _ => 0b11,
            }
        } else {
            match (dst, src2) {
                (
                    Some(
                        Operand::Mem(m)
                        | Operand::Segment(Segment {
                            address: m,
                            segment: _,
                        }),
                    ),
                    _,
                )
                | (
                    _,
                    Some(
                        Operand::Mem(m)
                        | Operand::Segment(Segment {
                            address: m,
                            segment: _,
                        }),
                    ),
                ) => {
                    if m.is_sib() {
                        if let Some((_, sz)) = m.offset_x86() {
                            if sz == 1 {
                                0b01
                            } else {
                                0b10
                            }
                        } else {
                            0b00
                        }
                    } else {
                        if let Some((_, sz)) = m.offset_x86() {
                            if sz == 1 {
                                0b01
                            } else {
                                0b10
                            }
                        } else {
                            0b00
                        }
                    }
                }
                _ => 0b11,
            }
        }
    };
    let mut modrm_reg_is_dst = modrm_reg_is_dst;

    let ssrc = src2;

    if matches!(ins.mnem, Ins::VINSERTF128) {
        modrm_reg_is_dst = true;
    }

    let reg = if let Some(reg) = reg {
        reg
    } else {
        if modrm_reg_is_dst {
            gen_rmreg(dst)
        } else {
            if let Some(Operand::Mem(_) | Operand::Segment(_)) = ssrc {
                modrm_reg_is_dst = true;
                gen_rmreg(dst)
            } else {
                gen_rmreg(ssrc)
            }
        }
    };
    let rm = {
        if ins.uses_sib() {
            0b100
        } else {
            if let Some(rm) = rm {
                rm
            } else {
                if modrm_reg_is_dst {
                    gen_rmreg(ssrc)
                } else {
                    gen_rmreg(dst)
                }
            }
        }
    };

    (mod_ << 6) + (reg << 3) + rm
}

fn gen_rmreg(op: Option<&Operand>) -> u8 {
    if op.is_none() {
        return 0;
    };
    match op.unwrap() {
        Operand::DbgReg(r) | Operand::CtrReg(r) | Operand::Reg(r) => r.to_byte(),
        Operand::Mem(m)
        | Operand::Segment(Segment {
            address: m,
            segment: _,
        }) => {
            if m.is_sib() {
                0b100
            } else {
                m.base().unwrap().to_byte()
            }
        }
        Operand::SegReg(r) => r.to_byte(),
        _ => 0,
    }
}

pub const fn vex2(vexr: bool, ssrc: u8, vlength: bool, pp: u8) -> [u8; 2] {
    [
        TWO_BYTE_PFX,
        ((vexr as u8) << 7 | ssrc << 3 | (vlength as u8) << 2 | pp),
    ]
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::shr::reg::Register;
    #[test]
    fn vex_asserts() {
        let reg = gen_vex4v(Some(&Operand::Reg(Register::XMM9)));
        println!("{:08b}", reg);
        assert!(gen_vex4v(Some(&Operand::Reg(Register::XMM9))) == 0b0110);
        let reg = gen_vex4v(Some(&Operand::Reg(Register::XMM0)));
        println!("{:08b}", reg);
        assert!(gen_vex4v(Some(&Operand::Reg(Register::XMM0))) == 0b1111);
    }
}
