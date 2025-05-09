// rasmx86_64 - src/core/comp.rs
// -----------------------------
// made by matissoss
// licensed under MPL 2.0

use std::borrow::Cow;

use crate::{
    core::{
        disp::gen_disp,
        mmx,
        modrm::gen_modrm,
        reloc::{RCategory, RType, Relocation},
        rex::gen_rex,
        sib::gen_sib,
        sse, sse2, sse3, sse4, ssse3,
    },
    shr::{
        ast::{IVariant, Instruction, Operand},
        ins::Mnemonic as Ins,
        num::Number,
        reg::Register,
        size::Size,
        symbol::{Symbol, SymbolType, Visibility},
        var::{VarContent, Variable},
    },
};

#[inline]
pub fn make_globals(symbols: &mut [Symbol], globals: &[String]) {
    for s in symbols {
        for g in globals {
            if s.name == Cow::Borrowed(g) {
                s.visibility = Visibility::Global;
                break;
            }
        }
    }
}
#[inline]
pub fn extern_trf(externs: &Vec<String>) -> Vec<Symbol> {
    let mut symbols = Vec::new();
    for extern_ in externs {
        symbols.push(Symbol {
            name: Cow::Borrowed(extern_),
            offset: 0,
            size: None,
            sindex: 0,
            stype: SymbolType::NoType,
            visibility: Visibility::Global,
            content: None,
            addend: 0,
            addt: 0,
        });
    }
    symbols
}

pub fn compile_section<'a>(
    vars: &'a Vec<&'a Variable<'a>>,
    sindex: u16,
    addt: u8,
) -> (Vec<u8>, Vec<Symbol<'a>>) {
    let mut buf: Vec<u8> = Vec::new();
    let mut symbols: Vec<Symbol> = Vec::new();

    let mut offset: u64 = 0;

    for v in vars {
        match v.content {
            VarContent::Uninit => {
                symbols.push(Symbol {
                    name: Cow::Borrowed(&v.name),
                    size: Some(v.size),
                    sindex,
                    stype: SymbolType::Object,
                    offset,
                    content: None,
                    visibility: v.visibility,
                    addend: 0,
                    addt,
                });
                offset += v.size as u64;
            }
            _ => {
                buf.extend(v.content.bytes());
                symbols.push(Symbol {
                    name: Cow::Borrowed(&v.name),
                    size: Some(v.size),
                    sindex,
                    stype: SymbolType::Object,
                    offset,
                    content: Some(Cow::Borrowed(&v.content)),
                    visibility: v.visibility,
                    addend: 0,
                    addt,
                });
                offset += v.size as u64;
            }
        }
    }
    (buf, symbols)
}

pub fn compile_label(lbl: &'_ Vec<Instruction>, bits: u8) -> (Vec<u8>, Vec<Relocation<'_>>) {
    let mut bytes = Vec::new();
    let mut reallocs = Vec::new();
    for ins in lbl {
        let res = compile_instruction(ins, bits);
        if let Some(mut rl) = res.1 {
            rl.offset += bytes.len() as u64;
            reallocs.push(rl);
        }
        bytes.extend(res.0);
    }
    (bytes, reallocs)
}

pub fn compile_instruction(ins: &'_ Instruction, bits: u8) -> (Vec<u8>, Option<Relocation<'_>>) {
    match ins.mnem {
        Ins::RET => (vec![0xC3], None),
        Ins::SYSCALL => (vec![0x0F, 0x05], None),
        Ins::PUSH => (ins_push(ins, bits), None),
        Ins::POP => (ins_pop(ins, bits), None),
        Ins::MOV => (ins_mov(ins, bits), None),
        Ins::ADD => (
            add_like_ins(
                ins,
                &[0x04, 0x05, 0x80, 0x81, 0x83, 0x00, 0x01, 0x02, 0x03],
                0,
                bits,
            ),
            None,
        ),
        Ins::OR => (
            add_like_ins(
                ins,
                &[0x0C, 0x0D, 0x80, 0x81, 0x83, 0x08, 0x09, 0x0A, 0x0B],
                1,
                bits,
            ),
            None,
        ),
        Ins::AND => (
            add_like_ins(
                ins,
                &[0x24, 0x25, 0x80, 0x81, 0x83, 0x20, 0x21, 0x22, 0x23],
                4,
                bits,
            ),
            None,
        ),
        Ins::SUB => (
            add_like_ins(
                ins,
                &[0x2C, 0x2D, 0x80, 0x81, 0x83, 0x28, 0x29, 0x2A, 0x2B],
                5,
                bits,
            ),
            None,
        ),
        Ins::XOR => (
            add_like_ins(
                ins,
                &[0x34, 0x35, 0x80, 0x81, 0x83, 0x30, 0x31, 0x32, 0x33],
                6,
                bits,
            ),
            None,
        ),
        Ins::SAL | Ins::SHL => (
            ins_shllike(ins, &[0xD0, 0xD2, 0xC0, 0xD1, 0xD3, 0xC1], 4, bits),
            None,
        ),
        Ins::SHR => (
            ins_shllike(ins, &[0xD0, 0xD2, 0xC0, 0xD1, 0xD3, 0xC1], 5, bits),
            None,
        ),
        Ins::SAR => (
            ins_shllike(ins, &[0xD0, 0xD2, 0xC0, 0xD1, 0xD3, 0xC1], 7, bits),
            None,
        ),
        Ins::TEST => (ins_test(ins, bits), None),
        Ins::INC => (ins_inclike(ins, &[0xFE, 0xFF], 0, bits), None),
        Ins::DEC => (ins_inclike(ins, &[0xFE, 0xFF], 1, bits), None),
        Ins::NOT => (ins_inclike(ins, &[0xF6, 0xF7], 2, bits), None),
        Ins::NEG => (ins_inclike(ins, &[0xF6, 0xF7], 3, bits), None),
        Ins::CMP => (ins_cmp(ins, bits), None),
        Ins::IMUL => (ins_imul(ins, bits), None),
        Ins::DIV => (ins_divmul(ins, 6, bits), None),
        Ins::IDIV => (ins_divmul(ins, 7, bits), None),
        Ins::MUL => (ins_divmul(ins, 4, bits), None),
        Ins::JMP => ins_jmplike(ins, [vec![0xE9], vec![0xFF]], 4, bits),
        Ins::CALL => ins_jmplike(ins, [vec![0xE8], vec![0xFF]], 2, bits),
        Ins::JE | Ins::JZ => ins_jmplike(ins, [vec![0x0F, 0x84], vec![]], 0, bits),
        Ins::JNE | Ins::JNZ => ins_jmplike(ins, [vec![0xFF, 0x85], vec![]], 0, bits),
        Ins::JL => ins_jmplike(ins, [vec![0x0F, 0x8C], vec![]], 0, bits),
        Ins::JLE => ins_jmplike(ins, [vec![0x0F, 0x8E], vec![]], 0, bits),
        Ins::JG => ins_jmplike(ins, [vec![0x0F, 0x8F], vec![]], 0, bits),
        Ins::JGE => ins_jmplike(ins, [vec![0x0F, 0x8D], vec![]], 0, bits),

        Ins::LEA => ins_lea(ins, bits),

        Ins::NOP => (vec![0x90], None),

        Ins::PUSHF | Ins::PUSHFD | Ins::PUSHFQ => (vec![0x9C], None),
        Ins::POPF | Ins::POPFD | Ins::POPFQ => (vec![0x9D], None),

        Ins::CLFLUSH => (
            gen_ins(ins, &[0x0F, 0xAE], (true, Some(7), None), None, bits, false),
            None,
        ),

        Ins::PAUSE => (vec![0xF3, 0x90], None),
        Ins::MWAIT => (vec![0x0F, 0x01, 0xC9], None),

        // SSE
        Ins::MOVSS => (
            sse::gen_movxxs(ins, bits, &[0xF3, 0x0F, 0x10], &[0xF3, 0x0F, 0x11]),
            None,
        ),
        Ins::MOVHLPS => (sse::gen_movxxs(ins, bits, &[0x0F, 0x12], &[]), None),
        Ins::MOVLHPS => (sse::gen_movxxs(ins, bits, &[0x0F, 0x16], &[]), None),
        Ins::MOVAPS => (
            sse::gen_movxxs(ins, bits, &[0x0F, 0x28], &[0x0F, 0x29]),
            None,
        ),
        Ins::MOVUPS => (
            sse::gen_movxxs(ins, bits, &[0x0F, 0x10], &[0x0F, 0x11]),
            None,
        ),
        Ins::MOVLPS => (
            sse::gen_movxxs(ins, bits, &[0x0F, 0x12], &[0x0F, 0x13]),
            None,
        ),
        Ins::MOVHPS => (
            sse::gen_movxxs(ins, bits, &[0x0F, 0x16], &[0x0F, 0x17]),
            None,
        ),

        Ins::ADDPS => (sse::sgen_ins(ins, bits, true, &[0x0F, 0x58]), None),
        Ins::ADDSS => (sse::sgen_ins(ins, bits, false, &[0x0F, 0x58]), None),

        Ins::SUBPS => (sse::sgen_ins(ins, bits, true, &[0x0F, 0x5C]), None),
        Ins::SUBSS => (sse::sgen_ins(ins, bits, false, &[0x0F, 0x5C]), None),

        Ins::MULPS => (sse::sgen_ins(ins, bits, true, &[0x0F, 0x59]), None),
        Ins::MULSS => (sse::sgen_ins(ins, bits, false, &[0x0F, 0x59]), None),

        Ins::DIVPS => (sse::sgen_ins(ins, bits, true, &[0x0F, 0x5E]), None),
        Ins::DIVSS => (sse::sgen_ins(ins, bits, false, &[0x0F, 0x5E]), None),

        Ins::MINPS => (sse::sgen_ins(ins, bits, true, &[0x0F, 0x5D]), None),
        Ins::MINSS => (sse::sgen_ins(ins, bits, false, &[0x0F, 0x5D]), None),

        Ins::MAXPS => (sse::sgen_ins(ins, bits, true, &[0x0F, 0x5F]), None),
        Ins::MAXSS => (sse::sgen_ins(ins, bits, false, &[0x0F, 0x5F]), None),

        Ins::RSQRTPS => (sse::sgen_ins(ins, bits, true, &[0x0F, 0x52]), None),
        Ins::RSQRTSS => (sse::sgen_ins(ins, bits, false, &[0x0F, 0x52]), None),

        Ins::SHUFPS => (sse::sgen_ins(ins, bits, true, &[0x0F, 0xC6]), None),

        Ins::SQRTPS => (sse::sgen_ins(ins, bits, true, &[0x0F, 0x51]), None),
        Ins::SQRTSS => (sse::sgen_ins(ins, bits, false, &[0x0F, 0x51]), None),

        Ins::CMPPS => (sse::sgen_ins(ins, bits, true, &[0x0F, 0xC2]), None),
        Ins::CMPSS => (sse::sgen_ins(ins, bits, false, &[0x0F, 0xC2]), None),

        Ins::RCPPS => (sse::sgen_ins(ins, bits, true, &[0x0F, 0x53]), None),
        Ins::RCPSS => (sse::sgen_ins(ins, bits, false, &[0x0F, 0x53]), None),

        Ins::COMISS => (sse::sgen_ins(ins, bits, false, &[0x0F, 0x2F]), None),
        Ins::UCOMISS => (sse::sgen_ins(ins, bits, false, &[0x0F, 0x2E]), None),

        Ins::ORPS => (sse::sgen_ins(ins, bits, true, &[0x0F, 0x56]), None),
        Ins::ANDPS => (sse::sgen_ins(ins, bits, true, &[0x0F, 0x54]), None),
        Ins::ANDNPS => (sse::sgen_ins(ins, bits, true, &[0x0F, 0x55]), None),
        Ins::XORPS => (sse::sgen_ins(ins, bits, true, &[0x0F, 0x57]), None),

        Ins::UNPCKLPS => (sse::sgen_ins(ins, bits, true, &[0x0F, 0x14]), None),
        Ins::UNPCKHPS => (sse::sgen_ins(ins, bits, true, &[0x0F, 0x15]), None),

        Ins::CVTSI2SS => (sse::gen_cvt4x(ins, bits, &[0x0F, 0x2A]), None),
        Ins::CVTPS2PI => (
            gen_ins(ins, &[0x0F, 0x2D], (true, None, None), None, bits, false),
            None,
        ),
        Ins::CVTTPS2PI => (
            gen_ins(ins, &[0x0F, 0x2C], (true, None, None), None, bits, false),
            None,
        ),
        Ins::CVTPI2PS => (
            gen_ins(ins, &[0x0F, 0x2A], (true, None, None), None, bits, false),
            None,
        ),
        Ins::CVTSS2SI => (sse::gen_cvt4x(ins, bits, &[0x0F, 0x2D]), None),

        // SSE2
        Ins::MOVNTI => (
            gen_ins(ins, &[0x0F, 0xC3], (true, None, None), None, bits, false),
            None,
        ),

        Ins::MFENCE => (vec![0xF0, 0xAE, 0xF0], None),
        Ins::LFENCE => (vec![0xF0, 0xAE, 0xE8], None),

        Ins::MOVNTPD => (sse2::sgen_ins(ins, bits, true, &[0x0F, 0x2B]), None),
        Ins::MOVNTDQ => (sse2::sgen_ins(ins, bits, true, &[0x0F, 0xE7]), None),
        Ins::MOVAPD => (
            sse2::gen_movxxd(ins, bits, &[0x66, 0x0F, 0x28], &[0x66, 0x0F, 0x29]),
            None,
        ),
        Ins::MOVUPD => (
            sse2::gen_movxxd(ins, bits, &[0x66, 0x0F, 0x10], &[0x66, 0x0F, 0x11]),
            None,
        ),
        Ins::MOVLPD => (
            sse2::gen_movxxd(ins, bits, &[0x66, 0x0F, 0x12], &[0x66, 0x0F, 0x13]),
            None,
        ),
        Ins::MOVHPD => (
            sse2::gen_movxxd(ins, bits, &[0x66, 0x0F, 0x16], &[0x66, 0x0F, 0x17]),
            None,
        ),
        Ins::MOVSD => (
            sse2::gen_movxxd(ins, bits, &[0xF2, 0x0F, 0x10], &[0xF2, 0x0F, 0x11]),
            None,
        ),
        Ins::MOVDQA => (
            sse2::gen_movxxd(ins, bits, &[0x66, 0x0F, 0x6F], &[0x66, 0x0F, 0x7F]),
            None,
        ),
        Ins::MOVDQ2Q => (
            gen_ins(
                ins,
                &[0xF2, 0x0F, 0xD6],
                (true, None, None),
                None,
                bits,
                false,
            ),
            None,
        ),

        Ins::MOVMSKPD => (sse2::gen_movmskpd(ins, bits, &[0x0F, 0x50]), None),

        Ins::ADDPD => (sse2::sgen_ins(ins, bits, true, &[0x0F, 0x58]), None),
        Ins::ADDSD => (sse2::sgen_ins(ins, bits, false, &[0x0F, 0x58]), None),

        Ins::SUBPD => (sse2::sgen_ins(ins, bits, true, &[0x0F, 0x5C]), None),
        Ins::SUBSD => (sse2::sgen_ins(ins, bits, false, &[0x0F, 0x5C]), None),

        Ins::MULPD => (sse2::sgen_ins(ins, bits, true, &[0x0F, 0x59]), None),
        Ins::MULSD => (sse2::sgen_ins(ins, bits, false, &[0x0F, 0x59]), None),

        Ins::DIVPD => (sse2::sgen_ins(ins, bits, true, &[0x0F, 0x5E]), None),
        Ins::DIVSD => (sse2::sgen_ins(ins, bits, false, &[0x0F, 0x5E]), None),

        Ins::MINPD => (sse2::sgen_ins(ins, bits, true, &[0x0F, 0x5D]), None),
        Ins::MINSD => (sse2::sgen_ins(ins, bits, false, &[0x0F, 0x5D]), None),

        Ins::MAXPD => (sse2::sgen_ins(ins, bits, true, &[0x0F, 0x5F]), None),
        Ins::MAXSD => (sse2::sgen_ins(ins, bits, false, &[0x0F, 0x5F]), None),

        Ins::SQRTPD => (sse2::sgen_ins(ins, bits, true, &[0x0F, 0x51]), None),
        Ins::SQRTSD => (sse2::sgen_ins(ins, bits, false, &[0x0F, 0x51]), None),

        Ins::CMPPD => (sse2::sgen_ins(ins, bits, true, &[0x0F, 0xC2]), None),
        Ins::CMPSD => (sse2::sgen_ins(ins, bits, false, &[0x0F, 0xC2]), None),

        Ins::COMISD => (sse2::sgen_ins(ins, bits, false, &[0x0F, 0x2F]), None),
        Ins::UCOMISD => (sse2::sgen_ins(ins, bits, false, &[0x0F, 0x2E]), None),

        Ins::ORPD => (sse2::sgen_ins(ins, bits, true, &[0x0F, 0x56]), None),
        Ins::ANDPD => (sse2::sgen_ins(ins, bits, true, &[0x0F, 0x54]), None),
        Ins::ANDNPD => (sse2::sgen_ins(ins, bits, true, &[0x0F, 0x55]), None),
        Ins::XORPD => (sse2::sgen_ins(ins, bits, true, &[0x0F, 0x57]), None),

        Ins::CVTPD2PI => (
            gen_ins(
                ins,
                &[0x66, 0x0F, 0x2D],
                (true, None, None),
                None,
                bits,
                false,
            ),
            None,
        ),
        Ins::CVTPI2PD => (
            gen_ins(
                ins,
                &[0x66, 0x0F, 0x2A],
                (true, None, None),
                None,
                bits,
                false,
            ),
            None,
        ),
        Ins::CVTPS2DQ => (
            gen_ins(
                ins,
                &[0x66, 0x0F, 0x5B],
                (true, None, None),
                None,
                bits,
                false,
            ),
            None,
        ),
        Ins::CVTDQ2PS => (
            gen_ins(ins, &[0x0F, 0x5B], (true, None, None), None, bits, false),
            None,
        ),
        Ins::CVTDQ2PD => (sse2::sgen_ins(ins, bits, true, &[0x0F, 0xE6]), None),
        Ins::CVTSD2SI => (sse2::sgen_ins(ins, bits, true, &[0x66, 0x0F, 0x2D]), None),
        Ins::CVTSI2SD => (sse2::sgen_ins(ins, bits, false, &[0x0F, 0x2A]), None),
        Ins::CVTTPS2DQ => (
            gen_ins(
                ins,
                &[0xF3, 0x0F, 0x5B],
                (true, None, None),
                None,
                bits,
                false,
            ),
            None,
        ),
        Ins::CVTTSD2SI => (sse2::sgen_ins(ins, bits, false, &[0x66, 0x0F, 0x2C]), None),
        Ins::CVTTPD2PI => (
            gen_ins(
                ins,
                &[0x66, 0x0F, 0x2C],
                (true, None, None),
                None,
                bits,
                false,
            ),
            None,
        ),

        Ins::PSHUFLW => (sse2::ins_shuff(ins, bits, &[0xF2, 0x0F, 0x70]), None),
        Ins::PSHUFHW => (sse2::ins_shuff(ins, bits, &[0xF3, 0x0F, 0x70]), None),
        Ins::PSHUFD => (sse2::ins_shuff(ins, bits, &[0x66, 0x0F, 0x70]), None),

        Ins::PSLLDQ => (
            sse2::ins_shift(ins, bits, &[0x66, 0x0F, 0x73], &[0x66, 0x0F, 0x73], 7),
            None,
        ),
        Ins::PSRLDQ => (
            sse2::ins_shift(ins, bits, &[0x66, 0x0F, 0x73], &[0x66, 0x0F, 0x73], 3),
            None,
        ),
        Ins::PUNPCKHQDQ => (sse2::ins_unpck_h(ins, bits, &[0x66, 0x0F, 0x6D]), None),

        Ins::PUNPCKLQDQ => (sse2::ins_unpck_h(ins, bits, &[0x66, 0x0F, 0x6C]), None),
        // MMX/SSE2
        Ins::MOVD | Ins::MOVQ => {
            if ins.which_variant() == IVariant::MMX {
                (mmx::ins_movdq(ins, bits), None)
            } else {
                (sse2::ins_movdq(ins, bits), None)
            }
        }
        Ins::PADDB => {
            if ins.which_variant() == IVariant::MMX {
                (mmx::ins_paddx(ins, bits, 1), None)
            } else {
                (sse2::ins_paddx(ins, bits, 1), None)
            }
        }
        Ins::PADDW => {
            if ins.which_variant() == IVariant::MMX {
                (mmx::ins_paddx(ins, bits, 2), None)
            } else {
                (sse2::ins_paddx(ins, bits, 2), None)
            }
        }
        Ins::PADDD => {
            if ins.which_variant() == IVariant::MMX {
                (mmx::ins_paddx(ins, bits, 3), None)
            } else {
                (sse2::ins_paddx(ins, bits, 3), None)
            }
        }
        Ins::PADDQ => {
            if ins.which_variant() == IVariant::MMX {
                (mmx::ins_paddx(ins, bits, 4), None)
            } else {
                (sse2::ins_paddx(ins, bits, 4), None)
            }
        }

        Ins::PADDSB => {
            if ins.which_variant() == IVariant::MMX {
                (mmx::ins_paddsx(ins, bits, true), None)
            } else {
                (sse2::ins_paddsx(ins, bits, true), None)
            }
        }
        Ins::PADDSW => {
            if ins.which_variant() == IVariant::MMX {
                (mmx::ins_paddsx(ins, bits, false), None)
            } else {
                (sse2::ins_paddsx(ins, bits, false), None)
            }
        }

        Ins::PSUBB => {
            if ins.which_variant() == IVariant::MMX {
                (mmx::ins_psubx(ins, bits, 1), None)
            } else {
                (sse2::ins_psubx(ins, bits, 1), None)
            }
        }
        Ins::PSUBW => {
            if ins.which_variant() == IVariant::MMX {
                (mmx::ins_psubx(ins, bits, 2), None)
            } else {
                (sse2::ins_psubx(ins, bits, 2), None)
            }
        }
        Ins::PSUBD => {
            if ins.which_variant() == IVariant::MMX {
                (mmx::ins_psubx(ins, bits, 3), None)
            } else {
                (sse2::ins_psubx(ins, bits, 3), None)
            }
        }

        Ins::PSUBSB => {
            if ins.which_variant() == IVariant::MMX {
                (mmx::ins_psubsx(ins, bits, true), None)
            } else {
                (sse2::ins_psubsx(ins, bits, true), None)
            }
        }
        Ins::PSUBSW => {
            if ins.which_variant() == IVariant::MMX {
                (mmx::ins_psubsx(ins, bits, false), None)
            } else {
                (sse2::ins_psubsx(ins, bits, false), None)
            }
        }

        Ins::PMULLW => {
            if ins.which_variant() == IVariant::MMX {
                (mmx::ins_pmulx(ins, bits, true), None)
            } else {
                (sse2::ins_pmulx(ins, bits, true), None)
            }
        }
        Ins::PMULHW => {
            if ins.which_variant() == IVariant::MMX {
                (mmx::ins_pmulx(ins, bits, false), None)
            } else {
                (sse2::ins_pmulx(ins, bits, false), None)
            }
        }

        Ins::PMULUDQ => {
            if ins.which_variant() == IVariant::MMX {
                (
                    gen_ins(ins, &[0x0F, 0xF4], (true, None, None), None, bits, false),
                    None,
                )
            } else {
                (
                    gen_ins(
                        ins,
                        &[0x66, 0x0F, 0xF4],
                        (true, None, None),
                        None,
                        bits,
                        false,
                    ),
                    None,
                )
            }
        }

        Ins::PMADDWD => {
            if ins.which_variant() == IVariant::MMX {
                (mmx::ins_pmaddwd(ins, bits), None)
            } else {
                (sse2::ins_pmaddwd(ins, bits), None)
            }
        }

        Ins::PCMPEQB => {
            if ins.which_variant() == IVariant::MMX {
                (mmx::ins_cmp(ins, bits, 1), None)
            } else {
                (sse2::ins_cmp(ins, bits, 1), None)
            }
        }
        Ins::PCMPEQW => {
            if ins.which_variant() == IVariant::MMX {
                (mmx::ins_cmp(ins, bits, 2), None)
            } else {
                (sse2::ins_cmp(ins, bits, 2), None)
            }
        }
        Ins::PCMPEQD => {
            if ins.which_variant() == IVariant::MMX {
                (mmx::ins_cmp(ins, bits, 3), None)
            } else {
                (sse2::ins_cmp(ins, bits, 3), None)
            }
        }

        Ins::PCMPGTB => {
            if ins.which_variant() == IVariant::MMX {
                (mmx::ins_cmp(ins, bits, 4), None)
            } else {
                (sse2::ins_cmp(ins, bits, 4), None)
            }
        }
        Ins::PCMPGTW => {
            if ins.which_variant() == IVariant::MMX {
                (mmx::ins_cmp(ins, bits, 5), None)
            } else {
                (sse2::ins_cmp(ins, bits, 5), None)
            }
        }
        Ins::PCMPGTD => {
            if ins.which_variant() == IVariant::MMX {
                (mmx::ins_cmp(ins, bits, 6), None)
            } else {
                (sse2::ins_cmp(ins, bits, 6), None)
            }
        }

        Ins::PACKUSWB => {
            if ins.which_variant() == IVariant::MMX {
                (mmx::ins_pack(ins, bits, 1), None)
            } else {
                (sse2::ins_pack(ins, bits, 1), None)
            }
        }
        Ins::PACKSSWB => {
            if ins.which_variant() == IVariant::MMX {
                (mmx::ins_pack(ins, bits, 2), None)
            } else {
                (sse2::ins_pack(ins, bits, 2), None)
            }
        }
        Ins::PACKSSDW => {
            if ins.which_variant() == IVariant::MMX {
                (mmx::ins_pack(ins, bits, 3), None)
            } else {
                (sse2::ins_pack(ins, bits, 3), None)
            }
        }

        Ins::PUNPCKLBW => {
            if ins.which_variant() == IVariant::MMX {
                (mmx::ins_unpack(ins, bits, 1), None)
            } else {
                (sse2::ins_unpack(ins, bits, 1), None)
            }
        }
        Ins::PUNPCKLWD => {
            if ins.which_variant() == IVariant::MMX {
                (mmx::ins_unpack(ins, bits, 2), None)
            } else {
                (sse2::ins_unpack(ins, bits, 2), None)
            }
        }
        Ins::PUNPCKLDQ => {
            if ins.which_variant() == IVariant::MMX {
                (mmx::ins_unpack(ins, bits, 3), None)
            } else {
                (sse2::ins_unpack(ins, bits, 3), None)
            }
        }
        Ins::PUNPCKHBW => {
            if ins.which_variant() == IVariant::MMX {
                (mmx::ins_unpack(ins, bits, 4), None)
            } else {
                (sse2::ins_unpack(ins, bits, 4), None)
            }
        }
        Ins::PUNPCKHWD => {
            if ins.which_variant() == IVariant::MMX {
                (mmx::ins_unpack(ins, bits, 5), None)
            } else {
                (sse2::ins_unpack(ins, bits, 5), None)
            }
        }
        Ins::PUNPCKHDQ => {
            if ins.which_variant() == IVariant::MMX {
                (mmx::ins_unpack(ins, bits, 6), None)
            } else {
                (sse2::ins_unpack(ins, bits, 6), None)
            }
        }

        Ins::PSLLQ => {
            if ins.which_variant() == IVariant::MMX {
                (
                    mmx::ins_shift(ins, bits, &[0x0F, 0xF3], &[0x0F, 0x73], 6),
                    None,
                )
            } else {
                (
                    sse2::ins_shift(ins, bits, &[0x66, 0x0F, 0xF3], &[0x66, 0x0F, 0x73], 6),
                    None,
                )
            }
        }
        Ins::PSLLD => {
            if ins.which_variant() == IVariant::MMX {
                (
                    mmx::ins_shift(ins, bits, &[0x0F, 0xF2], &[0x0F, 0x72], 6),
                    None,
                )
            } else {
                (
                    sse2::ins_shift(ins, bits, &[0x66, 0x0F, 0xF2], &[0x66, 0x0F, 0x72], 6),
                    None,
                )
            }
        }
        Ins::PSLLW => {
            if ins.which_variant() == IVariant::MMX {
                (
                    mmx::ins_shift(ins, bits, &[0x0F, 0xF1], &[0x0F, 0x71], 6),
                    None,
                )
            } else {
                (
                    sse2::ins_shift(ins, bits, &[0x66, 0x0F, 0xF1], &[0x66, 0x0F, 0x71], 6),
                    None,
                )
            }
        }
        Ins::PSRLW => {
            if ins.which_variant() == IVariant::MMX {
                (
                    mmx::ins_shift(ins, bits, &[0x0F, 0xD1], &[0x0F, 0x71], 2),
                    None,
                )
            } else {
                (
                    sse2::ins_shift(ins, bits, &[0x66, 0x0F, 0xD1], &[0x66, 0x0F, 0x71], 2),
                    None,
                )
            }
        }
        Ins::PSRLD => {
            if ins.which_variant() == IVariant::MMX {
                (
                    mmx::ins_shift(ins, bits, &[0x0F, 0xD2], &[0x0F, 0x72], 2),
                    None,
                )
            } else {
                (
                    sse2::ins_shift(ins, bits, &[0x66, 0x0F, 0xD2], &[0x66, 0x0F, 0x72], 2),
                    None,
                )
            }
        }
        Ins::PSRLQ => {
            if ins.which_variant() == IVariant::MMX {
                (
                    mmx::ins_shift(ins, bits, &[0x0F, 0xD3], &[0x0F, 0x73], 2),
                    None,
                )
            } else {
                (
                    sse2::ins_shift(ins, bits, &[0x66, 0x0F, 0xD3], &[0x66, 0x0F, 0x73], 2),
                    None,
                )
            }
        }
        Ins::PSRAW => {
            if ins.which_variant() == IVariant::MMX {
                (
                    mmx::ins_shift(ins, bits, &[0x0F, 0xE1], &[0x0F, 0x71], 4),
                    None,
                )
            } else {
                (
                    sse2::ins_shift(ins, bits, &[0x66, 0x0F, 0xE1], &[0x66, 0x0F, 0x71], 4),
                    None,
                )
            }
        }
        Ins::PSRAD => {
            if ins.which_variant() == IVariant::MMX {
                (
                    mmx::ins_shift(ins, bits, &[0x0F, 0xE2], &[0x0F, 0x72], 4),
                    None,
                )
            } else {
                (
                    sse2::ins_shift(ins, bits, &[0x66, 0x0F, 0xE2], &[0x66, 0x0F, 0x72], 4),
                    None,
                )
            }
        }

        Ins::POR => {
            if ins.which_variant() == IVariant::MMX {
                (
                    gen_ins(ins, &[0x0F, 0xEB], (true, None, None), None, bits, false),
                    None,
                )
            } else {
                (
                    gen_ins(
                        ins,
                        &[0x66, 0x0F, 0xEB],
                        (true, None, None),
                        None,
                        bits,
                        false,
                    ),
                    None,
                )
            }
        }
        Ins::PAND => {
            if ins.which_variant() == IVariant::MMX {
                (
                    gen_ins(ins, &[0x0F, 0xDB], (true, None, None), None, bits, false),
                    None,
                )
            } else {
                (
                    gen_ins(
                        ins,
                        &[0x66, 0x0F, 0xDB],
                        (true, None, None),
                        None,
                        bits,
                        false,
                    ),
                    None,
                )
            }
        }
        Ins::PANDN => {
            if ins.which_variant() == IVariant::MMX {
                (
                    gen_ins(ins, &[0x0F, 0xDF], (true, None, None), None, bits, false),
                    None,
                )
            } else {
                (
                    gen_ins(
                        ins,
                        &[0x66, 0x0F, 0xDF],
                        (true, None, None),
                        None,
                        bits,
                        false,
                    ),
                    None,
                )
            }
        }
        Ins::PXOR => {
            if ins.which_variant() == IVariant::MMX {
                (
                    gen_ins(ins, &[0x0F, 0xEF], (true, None, None), None, bits, false),
                    None,
                )
            } else {
                (
                    gen_ins(
                        ins,
                        &[0x66, 0x0F, 0xEF],
                        (true, None, None),
                        None,
                        bits,
                        false,
                    ),
                    None,
                )
            }
        }
        Ins::EMMS => (vec![0x0F, 0x77], None),

        // sse3
        Ins::ADDSUBPD => (sse3::sgen_ins(ins, bits, true, &[0x0F, 0xD0]), None),
        Ins::ADDSUBPS => (sse3::sgen_ins(ins, bits, false, &[0x0F, 0xD0]), None),

        Ins::HADDPD => (sse3::sgen_ins(ins, bits, true, &[0x0F, 0x7C]), None),
        Ins::HADDPS => (sse3::sgen_ins(ins, bits, false, &[0x0F, 0x7C]), None),

        Ins::HSUBPD => (sse3::sgen_ins(ins, bits, true, &[0x0F, 0x7D]), None),
        Ins::HSUBPS => (sse3::sgen_ins(ins, bits, false, &[0x0F, 0x7D]), None),

        Ins::MOVSLDUP => (
            gen_ins(
                ins,
                &[0xF3, 0x0F, 0x12],
                (true, None, None),
                None,
                bits,
                false,
            ),
            None,
        ),
        Ins::MOVSHDUP => (
            gen_ins(
                ins,
                &[0xF3, 0x0F, 0x16],
                (true, None, None),
                None,
                bits,
                false,
            ),
            None,
        ),
        Ins::MOVDDUP => (
            gen_ins(
                ins,
                &[0xF2, 0x0F, 0x12],
                (true, None, None),
                None,
                bits,
                false,
            ),
            None,
        ),

        Ins::LDDQU => (
            gen_ins(
                ins,
                &[0xF2, 0x0F, 0xF0],
                (true, None, None),
                None,
                bits,
                false,
            ),
            None,
        ),

        Ins::MONITOR => (vec![0x0F, 0x01, 0xC8], None),

        // ssse3
        Ins::PABSB => {
            if ins.which_variant() == IVariant::XMM {
                (ssse3::sgen_ins(ins, bits, false, &[0x0F, 0x38, 0x1C]), None)
            } else {
                (
                    gen_ins(
                        ins,
                        &[0x0F, 0x38, 0x1C],
                        (true, None, None),
                        None,
                        bits,
                        false,
                    ),
                    None,
                )
            }
        }
        Ins::PABSW => {
            if ins.which_variant() == IVariant::XMM {
                (ssse3::sgen_ins(ins, bits, false, &[0x0F, 0x38, 0x1D]), None)
            } else {
                (
                    gen_ins(
                        ins,
                        &[0x0F, 0x38, 0x1D],
                        (true, None, None),
                        None,
                        bits,
                        false,
                    ),
                    None,
                )
            }
        }
        Ins::PABSD => {
            if ins.which_variant() == IVariant::XMM {
                (ssse3::sgen_ins(ins, bits, false, &[0x0F, 0x38, 0x1E]), None)
            } else {
                (
                    gen_ins(
                        ins,
                        &[0x0F, 0x38, 0x1E],
                        (true, None, None),
                        None,
                        bits,
                        false,
                    ),
                    None,
                )
            }
        }

        Ins::PSIGNB => {
            if ins.which_variant() == IVariant::XMM {
                (ssse3::sgen_ins(ins, bits, false, &[0x0F, 0x38, 0x08]), None)
            } else {
                (
                    gen_ins(
                        ins,
                        &[0x0F, 0x38, 0x08],
                        (true, None, None),
                        None,
                        bits,
                        false,
                    ),
                    None,
                )
            }
        }
        Ins::PSIGNW => {
            if ins.which_variant() == IVariant::XMM {
                (ssse3::sgen_ins(ins, bits, false, &[0x0F, 0x38, 0x09]), None)
            } else {
                (
                    gen_ins(
                        ins,
                        &[0x0F, 0x38, 0x09],
                        (true, None, None),
                        None,
                        bits,
                        false,
                    ),
                    None,
                )
            }
        }
        Ins::PSIGND => {
            if ins.which_variant() == IVariant::XMM {
                (ssse3::sgen_ins(ins, bits, false, &[0x0F, 0x38, 0x0A]), None)
            } else {
                (
                    gen_ins(
                        ins,
                        &[0x0F, 0x38, 0x0A],
                        (true, None, None),
                        None,
                        bits,
                        false,
                    ),
                    None,
                )
            }
        }

        Ins::PSHUFB => {
            if ins.which_variant() == IVariant::XMM {
                (ssse3::sgen_ins(ins, bits, false, &[0x0F, 0x38, 0x00]), None)
            } else {
                (
                    gen_ins(
                        ins,
                        &[0x0F, 0x38, 0x00],
                        (true, None, None),
                        None,
                        bits,
                        false,
                    ),
                    None,
                )
            }
        }
        Ins::PHADDW => {
            if ins.which_variant() == IVariant::XMM {
                (ssse3::sgen_ins(ins, bits, false, &[0x0F, 0x38, 0x01]), None)
            } else {
                (
                    gen_ins(
                        ins,
                        &[0x0F, 0x38, 0x01],
                        (true, None, None),
                        None,
                        bits,
                        false,
                    ),
                    None,
                )
            }
        }
        Ins::PHADDD => {
            if ins.which_variant() == IVariant::XMM {
                (ssse3::sgen_ins(ins, bits, false, &[0x0F, 0x38, 0x02]), None)
            } else {
                (
                    gen_ins(
                        ins,
                        &[0x0F, 0x38, 0x02],
                        (true, None, None),
                        None,
                        bits,
                        false,
                    ),
                    None,
                )
            }
        }
        Ins::PHADDSW => {
            if ins.which_variant() == IVariant::XMM {
                (ssse3::sgen_ins(ins, bits, false, &[0x0F, 0x38, 0x03]), None)
            } else {
                (
                    gen_ins(
                        ins,
                        &[0x0F, 0x38, 0x03],
                        (true, None, None),
                        None,
                        bits,
                        false,
                    ),
                    None,
                )
            }
        }
        Ins::PHSUBW => {
            if ins.which_variant() == IVariant::XMM {
                (ssse3::sgen_ins(ins, bits, false, &[0x0F, 0x38, 0x05]), None)
            } else {
                (
                    gen_ins(
                        ins,
                        &[0x0F, 0x38, 0x05],
                        (true, None, None),
                        None,
                        bits,
                        false,
                    ),
                    None,
                )
            }
        }
        Ins::PHSUBD => {
            if ins.which_variant() == IVariant::XMM {
                (ssse3::sgen_ins(ins, bits, false, &[0x0F, 0x38, 0x06]), None)
            } else {
                (
                    gen_ins(
                        ins,
                        &[0x0F, 0x38, 0x06],
                        (true, None, None),
                        None,
                        bits,
                        false,
                    ),
                    None,
                )
            }
        }
        Ins::PHSUBSW => {
            if ins.which_variant() == IVariant::XMM {
                (ssse3::sgen_ins(ins, bits, false, &[0x0F, 0x38, 0x07]), None)
            } else {
                (
                    gen_ins(
                        ins,
                        &[0x0F, 0x38, 0x07],
                        (true, None, None),
                        None,
                        bits,
                        false,
                    ),
                    None,
                )
            }
        }
        Ins::PALIGNR => {
            if ins.which_variant() == IVariant::XMM {
                (
                    ssse3::ins_palignr(ins, bits, &[0x66, 0x0F, 0x3A, 0x0F]),
                    None,
                )
            } else {
                (ssse3::ins_palignr(ins, bits, &[0x0F, 0x3A, 0x0F]), None)
            }
        }
        Ins::PMULHRSW => {
            if ins.which_variant() == IVariant::XMM {
                (ssse3::sgen_ins(ins, bits, false, &[0x0F, 0x38, 0x0B]), None)
            } else {
                (
                    gen_ins(
                        ins,
                        &[0x0F, 0x38, 0x0B],
                        (true, None, None),
                        None,
                        bits,
                        false,
                    ),
                    None,
                )
            }
        }
        Ins::PMADDUBSW => {
            if ins.which_variant() == IVariant::XMM {
                (ssse3::sgen_ins(ins, bits, false, &[0x0F, 0x38, 0x04]), None)
            } else {
                (
                    gen_ins(
                        ins,
                        &[0x0F, 0x38, 0x04],
                        (true, None, None),
                        None,
                        bits,
                        false,
                    ),
                    None,
                )
            }
        }
        // sse4
        Ins::DPPS => (sse4::sgen_ins(ins, bits, true, &[0x0F, 0x3A, 0x40]), None),
        Ins::DPPD => (sse4::sgen_ins(ins, bits, true, &[0x0F, 0x3A, 0x41]), None),
        Ins::PTEST => (sse4::sgen_ins(ins, bits, true, &[0x0F, 0x38, 0x17]), None),
        Ins::PEXTRW => (sse4::sgen_ins(ins, bits, true, &[0x0F, 0x3A, 0x15]), None),
        Ins::PEXTRB => (sse4::sgen_ins(ins, bits, true, &[0x0F, 0x3A, 0x14]), None),
        Ins::PEXTRD => (sse4::sgen_ins(ins, bits, true, &[0x0F, 0x3A, 0x16]), None),
        Ins::PEXTRQ => (sse4::sgen_ins(ins, bits, true, &[0x0F, 0x3A, 0x16]), None),
        Ins::PINSRB => (sse4::sgen_ins(ins, bits, true, &[0x0F, 0x3A, 0x20]), None),
        Ins::PINSRD => (sse4::sgen_ins(ins, bits, true, &[0x0F, 0x3A, 0x22]), None),
        Ins::PINSRQ => (sse4::sgen_ins(ins, bits, true, &[0x0F, 0x3A, 0x22]), None),
        Ins::PMAXSB => (sse4::sgen_ins(ins, bits, true, &[0x0F, 0x38, 0x3C]), None),
        Ins::PMAXSD => (sse4::sgen_ins(ins, bits, true, &[0x0F, 0x38, 0x3D]), None),
        Ins::PMAXUW => (sse4::sgen_ins(ins, bits, true, &[0x0F, 0x38, 0x3E]), None),
        Ins::PMINSB => (sse4::sgen_ins(ins, bits, true, &[0x0F, 0x38, 0x38]), None),
        Ins::PMINSD => (sse4::sgen_ins(ins, bits, true, &[0x0F, 0x38, 0x39]), None),
        Ins::PMINUW => (sse4::sgen_ins(ins, bits, true, &[0x0F, 0x38, 0x3A]), None),
        Ins::PMULDQ => (sse4::sgen_ins(ins, bits, true, &[0x0F, 0x38, 0x28]), None),
        Ins::PMULLD => (sse4::sgen_ins(ins, bits, true, &[0x0F, 0x38, 0x40]), None),
        Ins::BLENDPS => (sse4::sgen_ins(ins, bits, true, &[0x0F, 0x3A, 0x0C]), None),
        Ins::BLENDPD => (sse4::sgen_ins(ins, bits, true, &[0x0F, 0x3A, 0x0D]), None),
        Ins::PBLENDW => (sse4::sgen_ins(ins, bits, true, &[0x0F, 0x3A, 0x0E]), None),
        Ins::PCMPEQQ => (sse4::sgen_ins(ins, bits, true, &[0x0F, 0x38, 0x29]), None),
        Ins::ROUNDPS => (sse4::sgen_ins(ins, bits, true, &[0x0F, 0x3A, 0x08]), None),
        Ins::ROUNDPD => (sse4::sgen_ins(ins, bits, true, &[0x0F, 0x3A, 0x09]), None),
        Ins::ROUNDSS => (sse4::sgen_ins(ins, bits, true, &[0x0F, 0x3A, 0x0A]), None),
        Ins::ROUNDSD => (sse4::sgen_ins(ins, bits, true, &[0x0F, 0x3A, 0x0B]), None),
        Ins::MPSADBW => (sse4::sgen_ins(ins, bits, true, &[0x0F, 0x3A, 0x42]), None),
        Ins::PCMPGTQ => (sse4::sgen_ins(ins, bits, true, &[0x0F, 0x38, 0x37]), None),
        Ins::BLENDVPS => (sse4::sgen_ins(ins, bits, true, &[0x0F, 0x38, 0x14]), None),
        Ins::BLENDVPD => (sse4::sgen_ins(ins, bits, true, &[0x0F, 0x38, 0x15]), None),
        Ins::PBLENDVB => (sse4::sgen_ins(ins, bits, true, &[0x0F, 0x38, 0x10]), None),
        Ins::INSERTPS => (sse4::sgen_ins(ins, bits, true, &[0x0F, 0x3A, 0x21]), None),
        Ins::PACKUSDW => (sse4::sgen_ins(ins, bits, true, &[0x0F, 0x38, 0x2B]), None),
        Ins::MOVNTDQA => (sse4::sgen_ins(ins, bits, true, &[0x0F, 0x38, 0x2A]), None),
        Ins::PCMPESTRM => (sse4::sgen_ins(ins, bits, true, &[0x0F, 0x3A, 0x60]), None),
        Ins::PCMPESTRI => (sse4::sgen_ins(ins, bits, true, &[0x0F, 0x3A, 0x61]), None),
        Ins::PCMPISTRM => (sse4::sgen_ins(ins, bits, true, &[0x0F, 0x3A, 0x62]), None),
        Ins::PCMPISTRI => (sse4::sgen_ins(ins, bits, true, &[0x0F, 0x3A, 0x63]), None),
        Ins::EXTRACTPS => (sse4::sgen_ins(ins, bits, true, &[0x0F, 0x3A, 0x17]), None),
        Ins::PHMINPOSUW => (sse4::sgen_ins(ins, bits, true, &[0x0F, 0x38, 0x41]), None),
        Ins::CRC32 => (sse4::sgen_ins(ins, bits, false, &[0x0F, 0x38, 0xF0]), None),
        Ins::POPCNT => (sse4::sgen_ins_alt(ins, bits, 0xF3, &[0x0F, 0xB8]), None),

        _ => (Vec::new(), None),
    }
}

fn ins_pop(ins: &Instruction, bits: u8) -> Vec<u8> {
    match ins.dst().unwrap() {
        Operand::Reg(r) => gen_base(ins, &[0x58 + r.to_byte()], bits, false),
        Operand::SegReg(r) => match r {
            Register::DS => vec![0x1F],
            Register::ES => vec![0x07],
            Register::SS => vec![0x17],
            Register::FS => vec![0x0F, 0xA1],
            Register::GS => vec![0x0F, 0xA9],
            Register::CS => vec![0x90],
            _ => invalid(),
        },
        Operand::Mem(_) | Operand::Segment(_) => vec![0x8F, gen_modrm(ins, None, Some(0), false)],
        _ => invalid(),
    }
}

fn ins_push(ins: &Instruction, bits: u8) -> Vec<u8> {
    match ins.dst().unwrap() {
        Operand::Reg(r) => gen_base(ins, &[0x50 + r.to_byte()], bits, false),
        Operand::SegReg(r) => match r {
            Register::CS => vec![0x0E],
            Register::SS => vec![0x16],
            Register::DS => vec![0x1E],
            Register::ES => vec![0x06],
            Register::FS => vec![0x0F, 0xA0],
            Register::GS => vec![0x0F, 0xA8],
            _ => invalid(),
        },
        Operand::Imm(nb) => match nb.size() {
            Size::Byte => {
                let mut opc = vec![0x6A];
                opc.extend(nb.split_into_bytes());
                opc
            }
            Size::Word | Size::Dword => {
                let mut b = vec![0x68];
                let mut x = nb.split_into_bytes();
                extend_imm(&mut x, 4);
                b.extend(x);
                b
            }
            _ => invalid(),
        },
        Operand::Mem(_) | Operand::Segment(_) => {
            gen_ins(ins, &[0xFF], (true, Some(6), None), None, bits, false)
        }
        _ => invalid(),
    }
}

fn ins_mov(ins: &Instruction, bits: u8) -> Vec<u8> {
    let src = ins.src().unwrap();
    let dst = ins.dst().unwrap();
    if let Operand::Reg(r) = dst {
        match src {
            Operand::SegReg(_) => gen_ins(ins, &[0x8C], (true, None, None), None, bits, false),
            Operand::CtrReg(_) => {
                gen_ins(ins, &[0x0F, 0x20], (true, None, None), None, bits, false)
            }
            Operand::DbgReg(_) => {
                gen_ins(ins, &[0x0F, 0x21], (true, None, None), None, bits, false)
            }
            Operand::Imm(n) => {
                let size = dst.size();
                let opc = match size {
                    Size::Byte => 0xB0 + r.to_byte(),
                    Size::Word | Size::Dword | Size::Qword => 0xB8 + r.to_byte(),
                    _ => invalid(),
                };
                let mut imm = n.split_into_bytes();
                if size == Size::Qword {
                    extend_imm(&mut imm, 4);
                } else {
                    extend_imm(&mut imm, size.into());
                }
                let mut base = gen_base(ins, &[opc], bits, false);
                base.extend(imm);
                base
            }
            Operand::Reg(_) | Operand::Mem(_) | Operand::Segment(_) => {
                let opc = if let Operand::Reg(_) = src {
                    match dst.size() {
                        Size::Byte => 0x88,
                        Size::Word | Size::Dword | Size::Qword => 0x89,
                        _ => invalid(),
                    }
                } else {
                    match dst.size() {
                        Size::Byte => 0x8A,
                        Size::Word | Size::Dword | Size::Qword => 0x8B,
                        _ => invalid(),
                    }
                };
                gen_ins(ins, &[opc], (true, None, None), None, bits, false)
            }
            _ => invalid(),
        }
    } else if let Operand::CtrReg(_) = dst {
        gen_ins(ins, &[0x0F, 0x22], (true, None, None), None, bits, true)
    } else if let Operand::DbgReg(_) = dst {
        gen_ins(ins, &[0x0F, 0x23], (true, None, None), None, bits, true)
    } else if let Operand::SegReg(_) = dst {
        match src {
            Operand::Reg(_) | Operand::Mem(_) => {
                gen_ins(ins, &[0x8E], (true, None, None), None, bits, false)
            }
            _ => invalid(),
        }
    } else if let Operand::Mem(_) | Operand::Segment(_) = dst {
        match src {
            Operand::Reg(_) => {
                let opc = match dst.size() {
                    Size::Byte => 0x88,
                    Size::Word | Size::Dword | Size::Qword => 0x89,
                    _ => invalid(),
                };
                gen_ins(ins, &[opc], (true, None, None), None, bits, false)
            }
            Operand::Imm(n) => {
                let size = dst.size();
                let opc = match size {
                    Size::Byte => 0xC6,
                    Size::Word | Size::Dword | Size::Qword => 0xC7,
                    _ => invalid(),
                };
                let mut imm = n.split_into_bytes();
                extend_imm(&mut imm, size as u8 + 1);
                gen_ins(ins, &[opc], (true, Some(0), None), Some(imm), bits, false)
            }
            _ => invalid(),
        }
    } else {
        invalid()
    }
}

// opc[0]  = AL, imm8
// opc[1]  = AX/EAX/RAX, imm32
// opc[2]  = r/m8, imm8
// opc[3]  = r/m16/32/64, imm16/32
// opc[4] =  r/m16/32/64, imm8
// opc[5]  = r/m8, r8
// opc[6]  = r/m16/32/64, r16/32/64
// opc[7]  = r8, r/m8
// opc[8]  = r16/32/64, r/m16/32/64
fn add_like_ins(ins: &Instruction, opc: &[u8; 9], ovrreg: u8, bits: u8) -> Vec<u8> {
    let src = ins.src().unwrap();
    let dst = ins.dst().unwrap();

    match (dst, src) {
        (Operand::Reg(dstr), Operand::Imm(srci)) => {
            let mut imm = srci.split_into_bytes();
            if let Size::Dword | Size::Word = srci.size() {
                if let Register::RAX | Register::EAX = dstr {
                    extend_imm(&mut imm, 4);
                    return bs_imm(ins, &[opc[1]], &imm, bits, false);
                } else if let Register::AX = dstr {
                    extend_imm(&mut imm, 2);
                    return bs_imm(ins, &[opc[1]], &imm, bits, false);
                }
            }
            if let Register::AL = dstr {
                if let Size::Byte = srci.size() {
                    return bs_imm(ins, &[opc[0]], &imm, bits, false);
                }
            } else if let Register::AX = dstr {
                if let Size::Byte = srci.size() {
                    extend_imm(&mut imm, 2);
                    return bs_imm(ins, &[opc[1]], &imm, bits, false);
                }
            }

            let opc = match dstr.size() {
                Size::Byte => opc[2],
                Size::Dword | Size::Qword | Size::Word => {
                    if imm.len() == 1 {
                        opc[4]
                    } else {
                        opc[3]
                    }
                }
                _ => invalid(),
            };
            let mut base = gen_base(ins, &[opc], bits, false);
            base.push(gen_modrm(ins, Some(ovrreg), None, false));
            extend_imm(&mut imm, 1);
            base.extend(imm);
            base
        }
        (Operand::Segment(dstm), Operand::Imm(srci)) => {
            let mut imm = srci.split_into_bytes();
            let opc = match dstm.address.size() {
                Size::Byte => opc[2],
                Size::Word => opc[3],
                Size::Dword => opc[3],
                Size::Qword => {
                    if imm.len() == 1 {
                        opc[4]
                    } else {
                        opc[3]
                    }
                }
                _ => invalid(),
            };
            if let (Size::Word | Size::Byte, Size::Word) = (srci.size(), dstm.address.size()) {
                extend_imm(&mut imm, 2);
            } else if let (Size::Byte, Size::Dword) = (srci.size(), dstm.address.size()) {
                extend_imm(&mut imm, 4);
            } else if let (crate::shr::ins::Mnemonic::CMP, Size::Byte, Size::Qword) =
                (ins.mnem, srci.size(), dstm.address.size())
            {
                extend_imm(&mut imm, 4);
            } else if srci.size() != Size::Byte {
                extend_imm(&mut imm, 4);
            }

            gen_ins(
                ins,
                &[opc],
                (true, Some(ovrreg), None),
                Some(imm),
                bits,
                false,
            )
        }
        (Operand::Mem(dstm), Operand::Imm(srci)) => {
            let mut imm = srci.split_into_bytes();
            let opc = match dstm.size() {
                Size::Byte => opc[2],
                Size::Word => opc[3],
                Size::Dword => opc[3],
                Size::Qword => {
                    if imm.len() == 1 {
                        opc[4]
                    } else {
                        opc[3]
                    }
                }
                _ => invalid(),
            };
            if let (Size::Word | Size::Byte, Size::Word) = (srci.size(), dstm.size()) {
                extend_imm(&mut imm, 2);
            } else if let (Size::Byte, Size::Dword) = (srci.size(), dstm.size()) {
                extend_imm(&mut imm, 4);
            } else if let (crate::shr::ins::Mnemonic::CMP, Size::Byte, Size::Qword) =
                (ins.mnem, srci.size(), dstm.size())
            {
                extend_imm(&mut imm, 4);
            } else if srci.size() != Size::Byte {
                extend_imm(&mut imm, 4);
            }

            gen_ins(
                ins,
                &[opc],
                (true, Some(ovrreg), None),
                Some(imm),
                bits,
                false,
            )
        }
        (Operand::Reg(r), Operand::Segment(_) | Operand::Mem(_) | Operand::Reg(_)) => {
            let opc = match r.size() {
                Size::Byte => opc[7],
                Size::Word | Size::Dword | Size::Qword => opc[6],
                _ => invalid(),
            };
            gen_ins(ins, &[opc], (true, None, None), None, bits, false)
        }
        (Operand::Segment(m), Operand::Reg(_)) => {
            let opc = match m.address.size() {
                Size::Byte => opc[7],
                Size::Word | Size::Dword | Size::Qword => opc[6],
                _ => invalid(),
            };
            gen_ins(ins, &[opc], (true, None, None), None, bits, false)
        }
        (Operand::Mem(m), Operand::Reg(_)) => {
            let opc = match m.size() {
                Size::Byte => opc[7],
                Size::Word | Size::Dword | Size::Qword => opc[6],
                _ => invalid(),
            };
            gen_ins(ins, &[opc], (true, None, None), None, bits, false)
        }
        _ => invalid(),
    }
}

fn ins_cmp(ins: &Instruction, bits: u8) -> Vec<u8> {
    let src = ins.src().unwrap();
    let dst = ins.dst().unwrap();

    match (dst, src) {
        (Operand::Reg(dstr), Operand::Imm(srci)) => {
            let mut imm = srci.split_into_bytes();
            if let Size::Dword | Size::Word = srci.size() {
                if let Register::RAX | Register::EAX = dstr {
                    extend_imm(&mut imm, 4);
                    return bs_imm(ins, &[0x3D], &imm, bits, false);
                } else if let Register::AX = dstr {
                    extend_imm(&mut imm, 2);
                    return bs_imm(ins, &[0x3D], &imm, bits, false);
                }
            }
            if let Register::AL = dstr {
                if let Size::Byte = srci.size() {
                    return bs_imm(ins, &[0x3C], &imm, bits, false);
                }
            } else if let Register::AX = dstr {
                if let Size::Byte = srci.size() {
                    extend_imm(&mut imm, 2);
                    return bs_imm(ins, &[0x3D], &imm, bits, false);
                }
            }

            let opc = match dstr.size() {
                Size::Byte => 0x80,
                Size::Dword | Size::Qword | Size::Word => {
                    if imm.len() == 1 {
                        if imm[0] <= 127 {
                            0x83
                        } else {
                            0x80
                        }
                    } else {
                        0x80
                    }
                }
                _ => invalid(),
            };
            let mut base = gen_base(ins, &[opc], bits, false);
            base.push(gen_modrm(ins, Some(7), None, false));
            extend_imm(&mut imm, 1);
            base.extend(imm);
            base
        }
        (Operand::Segment(dstm), Operand::Imm(srci)) => {
            let mut imm = srci.split_into_bytes();
            let opc = match dstm.address.size() {
                Size::Byte => 0x80,
                Size::Qword | Size::Word | Size::Dword => {
                    if imm.len() == 1 {
                        if imm[0] <= 127 {
                            0x83
                        } else {
                            0x81
                        }
                    } else {
                        0x81
                    }
                }
                _ => invalid(),
            };
            if let (Size::Word | Size::Byte, Size::Word) = (srci.size(), dstm.address.size()) {
                extend_imm(&mut imm, 2);
            } else if let (Size::Byte, Size::Dword | Size::Qword) =
                (srci.size(), dstm.address.size())
            {
                extend_imm(&mut imm, 4);
            } else if srci.size() != Size::Byte {
                extend_imm(&mut imm, 4);
            }

            gen_ins(ins, &[opc], (true, Some(7), None), Some(imm), bits, false)
        }
        (Operand::Mem(dstm), Operand::Imm(srci)) => {
            let mut imm = srci.split_into_bytes();
            let opc = match dstm.size() {
                Size::Byte => 0x80,
                Size::Qword | Size::Word | Size::Dword => {
                    if imm.len() == 1 {
                        if imm[0] <= 127 {
                            0x83
                        } else {
                            0x81
                        }
                    } else {
                        0x81
                    }
                }
                _ => invalid(),
            };
            if let (Size::Word | Size::Byte, Size::Word) = (srci.size(), dstm.size()) {
                extend_imm(&mut imm, 2);
            } else if let (Size::Byte, Size::Dword | Size::Qword) = (srci.size(), dstm.size()) {
                extend_imm(&mut imm, 4);
            } else if srci.size() != Size::Byte {
                extend_imm(&mut imm, 4);
            }

            gen_ins(ins, &[opc], (true, Some(7), None), Some(imm), bits, false)
        }
        (Operand::Reg(r), Operand::Segment(_) | Operand::Mem(_) | Operand::Reg(_)) => {
            let opc = match r.size() {
                Size::Byte => 0x3A,
                Size::Word | Size::Dword | Size::Qword => 0x3B,
                _ => invalid(),
            };
            gen_ins(ins, &[opc], (true, None, None), None, bits, false)
        }
        (Operand::Mem(m), Operand::Reg(_)) => {
            let opc = match m.size() {
                Size::Byte => 0x38,
                Size::Word | Size::Dword | Size::Qword => 0x39,
                _ => invalid(),
            };
            gen_ins(ins, &[opc], (true, None, None), None, bits, false)
        }
        (Operand::Segment(m), Operand::Reg(_)) => {
            let opc = match m.address.size() {
                Size::Byte => 0x38,
                Size::Word | Size::Dword | Size::Qword => 0x39,
                _ => invalid(),
            };
            gen_ins(ins, &[opc], (true, None, None), None, bits, false)
        }
        _ => invalid(),
    }
}

fn ins_test(ins: &Instruction, bits: u8) -> Vec<u8> {
    let src = ins.src().unwrap();
    let dst = ins.dst().unwrap();

    match (dst, src) {
        (Operand::Reg(dstr), Operand::Imm(srci)) => {
            let mut imm = srci.split_into_bytes();
            if let Size::Dword | Size::Word = srci.size() {
                if let Register::RAX | Register::EAX = dstr {
                    extend_imm(&mut imm, 4);
                    return bs_imm(ins, &[0xA9], &imm, bits, false);
                } else if let Register::AX = dstr {
                    extend_imm(&mut imm, 2);
                    return bs_imm(ins, &[0xA9], &imm, bits, false);
                }
            }
            if let Register::AL = dstr {
                if let Size::Byte = srci.size() {
                    return bs_imm(ins, &[0xA8], &imm, bits, false);
                }
            } else if let Register::AX = dstr {
                if let Size::Byte = srci.size() {
                    extend_imm(&mut imm, 2);
                    return bs_imm(ins, &[0xA9], &imm, bits, false);
                }
            }

            let opc = match dstr.size() {
                Size::Byte => 0xF6,
                Size::Dword | Size::Qword | Size::Word => 0xF7,
                _ => invalid(),
            };
            let mut base = gen_base(ins, &[opc], bits, false);
            base.push(gen_modrm(ins, Some(0), None, false));
            extend_imm(&mut imm, 1);
            base.extend(imm);
            base
        }
        (Operand::Segment(dsts), Operand::Imm(srci)) => {
            let mut imm = srci.split_into_bytes();
            let opc = match dsts.address.size() {
                Size::Byte => 0xF6,
                Size::Qword | Size::Word | Size::Dword => 0xF7,
                _ => invalid(),
            };
            if let (Size::Word | Size::Byte, Size::Word) = (srci.size(), dsts.address.size()) {
                extend_imm(&mut imm, 2);
            } else if let (Size::Byte, Size::Dword | Size::Qword) =
                (srci.size(), dsts.address.size())
            {
                extend_imm(&mut imm, 4);
            } else if srci.size() != Size::Byte {
                extend_imm(&mut imm, 4);
            }

            gen_ins(ins, &[opc], (true, Some(0), None), Some(imm), bits, false)
        }
        (Operand::Mem(dstm), Operand::Imm(srci)) => {
            let mut imm = srci.split_into_bytes();
            let opc = match dstm.size() {
                Size::Byte => 0xF6,
                Size::Qword | Size::Word | Size::Dword => 0xF7,
                _ => invalid(),
            };
            if let (Size::Word | Size::Byte, Size::Word) = (srci.size(), dstm.size()) {
                extend_imm(&mut imm, 2);
            } else if let (Size::Byte, Size::Dword | Size::Qword) = (srci.size(), dstm.size()) {
                extend_imm(&mut imm, 4);
            } else if srci.size() != Size::Byte {
                extend_imm(&mut imm, 4);
            }

            gen_ins(ins, &[opc], (true, Some(0), None), Some(imm), bits, false)
        }
        (Operand::Reg(_) | Operand::Mem(_) | Operand::Segment(_), Operand::Reg(_)) => {
            let opc = match dst.size() {
                Size::Byte => 0x84,
                Size::Word | Size::Dword | Size::Qword => 0x85,
                _ => invalid(),
            };
            gen_ins(ins, &[opc], (true, None, None), None, bits, false)
        }
        _ => invalid(),
    }
}

fn ins_imul(ins: &Instruction, bits: u8) -> Vec<u8> {
    match ins.src() {
        None => {
            let opc = match ins.dst().unwrap().size() {
                Size::Byte => &[0xF6],
                _ => &[0xF7],
            };
            gen_ins(ins, opc, (true, Some(5), None), None, bits, false)
        }
        Some(_) => match ins.oprs.get(2) {
            Some(Operand::Imm(imm)) => {
                let (opc, size) = match imm.size() {
                    Size::Byte => (0x6B, 1),
                    Size::Word => (0x69, 2),
                    _ => (0x69, 4),
                };
                let mut imm_b = imm.split_into_bytes();
                extend_imm(&mut imm_b, size);
                let (dst, src) = if let (Some(Operand::Reg(r)), Some(Operand::Reg(r1))) =
                    (ins.dst(), ins.src())
                {
                    (Some(r.to_byte()), Some(r1.to_byte()))
                } else {
                    (None, None)
                };
                gen_ins(ins, &[opc], (true, dst, src), Some(imm_b), bits, false)
            }
            _ => gen_ins(ins, &[0x0F, 0xAF], (true, None, None), None, bits, false),
        },
    }
}

// opc[0] = r/m8, 1
// opc[1] = r/m8, cl
// opc[2] = r/m8, imm8
// opc[3] = r/m16/32/64, 1
// opc[4] = r/m16/32/64, cl
// opc[5] = r/m16/32/64, imm8
fn ins_shllike(ins: &Instruction, opc: &[u8; 6], ovr: u8, bits: u8) -> Vec<u8> {
    let src = ins.src().unwrap();
    let dst = ins.dst().unwrap();
    let (opcd, imm) = match src {
        Operand::Reg(Register::CL) => match dst.size() {
            Size::Byte => (opc[1], None),
            Size::Word | Size::Dword | Size::Qword => (opc[4], None),
            _ => invalid(),
        },
        Operand::Imm(Number::UInt8(1) | Number::Int8(1)) => match dst.size() {
            Size::Byte => (opc[0], None),
            Size::Word | Size::Dword | Size::Qword => (opc[3], None),
            _ => invalid(),
        },
        Operand::Imm(imm) => match dst.size() {
            Size::Byte => (opc[2], Some(imm.split_into_bytes())),
            Size::Word | Size::Dword | Size::Qword => (opc[5], Some(imm.split_into_bytes())),
            _ => invalid(),
        },
        _ => invalid(),
    };
    let mut base = if dst.size() == Size::Word {
        vec![0x66]
    } else {
        vec![]
    };
    let gen_b = gen_base(ins, &[opcd], bits, false);
    if gen_b[0] == 0x66 {
        base = gen_b;
    } else {
        base.extend(gen_b);
    }
    base.push(gen_modrm(ins, Some(ovr), None, false));
    if let Some(sib) = gen_sib(dst) {
        base.push(sib);
    }
    if let Some(dsp) = gen_disp(dst) {
        base.extend(dsp);
    }
    if let Some(imm) = imm {
        base.extend(imm);
    }
    base
}

fn ins_inclike(ins: &Instruction, opc: &[u8; 2], ovr: u8, bits: u8) -> Vec<u8> {
    let opc = match ins.dst().unwrap().size() {
        Size::Byte => opc[0],
        _ => opc[1],
    };
    gen_ins(ins, &[opc], (true, Some(ovr), None), None, bits, false)
}

fn ins_lea(ins: &Instruction, bits: u8) -> (Vec<u8>, Option<Relocation>) {
    let mut base = gen_base(ins, &[0x8D], bits, false);
    let modrm = if let Operand::Reg(r) = ins.dst().unwrap() {
        0b100 + (r.to_byte() << 3)
    } else {
        0
    };
    base.push(modrm);
    base.push(0x25);
    let symbol = match ins.src().unwrap() {
        Operand::SymbolRef(s) => s,
        _ => invalid(),
    };
    let blen = base.len();
    base.extend([0x00; 4]);
    (
        base,
        Some(Relocation {
            rtype: RType::S32,
            symbol: Cow::Owned(symbol),
            offset: blen as u64,
            addend: 0,
            size: 4,
            catg: RCategory::Lea,
        }),
    )
}

// opc = opcode ONLY for rel32
// why? because i'm too lazy to implement other rel's
//
// opc[0] = rel32
// opc[1] = r/m
fn ins_jmplike(
    ins: &Instruction,
    opc: [Vec<u8>; 2],
    addt: u8,
    bits: u8,
) -> (Vec<u8>, Option<Relocation>) {
    match ins.dst().unwrap() {
        Operand::SymbolRef(s) => {
            let rel = Relocation {
                rtype: RType::PCRel32,
                symbol: Cow::Owned(s),
                addend: -4,
                offset: opc[0].len() as u64,
                size: 4,
                catg: RCategory::Jump,
            };
            let mut opc = opc[0].clone();
            opc.extend([0; 4]);
            (opc, Some(rel))
        }
        Operand::Reg(_) | Operand::Mem(_) => (
            gen_ins(ins, &opc[1], (true, Some(addt), None), None, bits, false),
            None,
        ),
        _ => invalid(),
    }
}

fn ins_divmul(ins: &Instruction, ovr: u8, bits: u8) -> Vec<u8> {
    let opc = match ins.dst().unwrap().size() {
        Size::Byte => [0xF6],
        _ => [0xF7],
    };
    gen_ins(ins, &opc, (true, Some(ovr), None), None, bits, false)
}

// ==============================
// Utils

pub fn bs_imm(ins: &Instruction, opc: &[u8], imm: &[u8], bits: u8, rev: bool) -> Vec<u8> {
    let mut b = gen_base(ins, opc, bits, rev);
    b.extend(imm);
    b
}

pub fn extend_imm(imm: &mut Vec<u8>, size: u8) {
    let size = size as usize;
    while imm.len() < size {
        imm.push(0)
    }
}

pub fn gen_ins_wpref(
    ins: &Instruction,
    opc: &[u8],
    modrm: (bool, Option<u8>, Option<u8>),
    imm: Option<Vec<u8>>,
    pref: u8,
    bits: u8,
    rev: bool,
) -> Vec<u8> {
    let mut base = vec![pref];
    base.extend(gen_base(ins, opc, bits, rev));
    if modrm.0 {
        base.push(gen_modrm(ins, modrm.1, modrm.2, rev));

        if let Some(dst) = ins.dst() {
            if let Some(sib) = gen_sib(dst) {
                base.push(sib);
            }
        }
        if let Some(src) = ins.src() {
            if let Some(sib) = gen_sib(src) {
                base.push(sib);
            }
        }
    }

    if let Some(dst) = ins.dst() {
        if let Some(disp) = gen_disp(dst) {
            base.extend(disp);
        }
    }
    if let Some(src) = ins.src() {
        if let Some(disp) = gen_disp(src) {
            base.extend(disp);
        }
    }
    if let Some(imm) = imm {
        base.extend(imm);
    }
    base
}

pub fn gen_ins(
    ins: &Instruction,
    opc: &[u8],
    modrm: (bool, Option<u8>, Option<u8>),
    imm: Option<Vec<u8>>,
    bits: u8,
    rev: bool,
) -> Vec<u8> {
    let mut base = gen_base(ins, opc, bits, rev);
    if modrm.0 {
        base.push(gen_modrm(ins, modrm.1, modrm.2, rev));

        if let Some(dst) = ins.dst() {
            if let Some(sib) = gen_sib(dst) {
                base.push(sib);
            }
        }
        if let Some(src) = ins.src() {
            if let Some(sib) = gen_sib(src) {
                base.push(sib);
            }
        }
    }

    if let Some(dst) = ins.dst() {
        if let Some(disp) = gen_disp(dst) {
            base.extend(disp);
        }
    }
    if let Some(src) = ins.src() {
        if let Some(disp) = gen_disp(src) {
            base.extend(disp);
        }
    }
    if let Some(imm) = imm {
        base.extend(imm);
    }
    base
}

pub fn gen_base(ins: &Instruction, opc: &[u8], bits: u8, rev: bool) -> Vec<u8> {
    // how does this even work? (probably doesn't)
    let (rex_bool, rex) = if bits == 64 {
        if let Some(rex) = gen_rex(ins, rev) {
            (rex & 0x08 == 8, Some(rex))
        } else {
            (ins.size() == Size::Qword || ins.size() == Size::Any, None)
        }
    } else {
        (false, None)
    };

    let mut used_66 = false;
    let mut size_ovr = if let Some(dst) = ins.dst() {
        if let Some(s) = gen_size_ovr(ins, dst, bits, rex_bool) {
            used_66 = s == 0x66;
            vec![s]
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };

    if let Some(src) = ins.src() {
        if let Some(s) = gen_size_ovr(ins, src, bits, rex_bool) {
            if !used_66 && !s == 0x66 {
                size_ovr.push(s);
            }
        }
    }
    let mut base = size_ovr;

    if let Some(s) = gen_segm_pref(ins) {
        base.push(s);
    }

    if let Some(rex) = rex {
        base.push(rex);
    }

    base.extend(opc);
    base
}

fn gen_size_ovr(ins: &Instruction, op: &Operand, bits: u8, rexw: bool) -> Option<u8> {
    let (size, is_mem) = match op {
        Operand::Reg(r) => (r.size(), false),
        Operand::CtrReg(r) => (r.size(), false),
        Operand::Mem(m) => (m.size(), false),
        Operand::Segment(s) => (s.address.size(), true),
        _ => return None,
    };
    if size == Size::Byte || size == Size::Xword {
        return None;
    }
    match bits {
        16 => match (size, is_mem) {
            (Size::Word, _) => None,
            (Size::Dword, true) => Some(0x67),
            (Size::Dword, false) => Some(0x66),
            _ => inv_osop(&format!("{:?}", op)),
        },
        32 => match (size, is_mem) {
            (Size::Word, false) => Some(0x66),
            (Size::Word, true) => Some(0x67),
            (Size::Dword, _) => None,
            _ => inv_osop(&format!("{:?}", op)),
        },
        64 => match (size, is_mem) {
            (Size::Qword, false) => {
                if ins.mnem.defaults_to_64bit() || rexw || ins.uses_cr() || ins.uses_dr() {
                    None
                } else {
                    Some(0x66)
                }
            }
            (Size::Dword, false) | (Size::Qword, true) => None,
            (Size::Word, false) => Some(0x66),
            (Size::Word, true) => Some(0x67),
            (Size::Dword, true) => Some(0x67),
            _ => inv_osop(&format!("{:?}", op)),
        },
        _ => invalid(),
    }
}

fn gen_segm_pref(ins: &Instruction) -> Option<u8> {
    if let Some(d) = ins.dst() {
        if let Some(s) = gen_segm_pref_op(d) {
            return Some(s);
        }
    }
    if let Some(d) = ins.src() {
        if let Some(s) = gen_segm_pref_op(d) {
            return Some(s);
        }
    }
    None
}

fn gen_segm_pref_op(op: &Operand) -> Option<u8> {
    if let Operand::Segment(s) = op {
        match s.segment {
            Register::CS => Some(0x2E),
            Register::SS => Some(0x36),
            Register::DS => Some(0x3E),
            Register::ES => Some(0x26),
            Register::FS => Some(0x64),
            Register::GS => Some(0x65),
            _ => None,
        }
    } else {
        None
    }
}

fn inv_osop(s: &str) -> ! {
    panic!("comp.rs:gen_size_ovr+1 {}", s)
}

fn invalid() -> ! {
    panic!("Unexpected thing that should not happen")
}
