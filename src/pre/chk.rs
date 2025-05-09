// rasmx86_64 - src/pre/chk.rs
// ---------------------------
// made by matissoss
// licensed under MPL 2.0

use crate::core::rex::gen_rex;
use crate::shr::{
    ast::{Instruction, Operand, AST},
    atype::*,
    error::RASMError,
    ins::Mnemonic as Mnm,
    num::Number,
    reg::{Purpose as RPurpose, Register},
    size::Size,
};

pub fn check_ast(file: &AST) -> Option<Vec<(String, Vec<RASMError>)>> {
    let mut errors: Vec<(String, Vec<RASMError>)> = Vec::new();

    let chk_ins: fn(&Instruction) -> Option<RASMError> = match file.bits {
        Some(64) => check_ins64bit,
        _ => check_ins32bit,
    };

    for label in &file.labels {
        let mut errs = Vec::new();
        for inst in &label.inst {
            if let Some(mut err) = chk_ins(inst) {
                err.set_line(inst.line);
                errs.push(err);
            }
        }
        if !errs.is_empty() {
            errors.push((label.name.to_string(), errs));
        }
    }

    if errors.is_empty() {
        None
    } else {
        Some(errors)
    }
}

fn check_ins32bit(ins: &Instruction) -> Option<RASMError> {
    if gen_rex(ins, false).is_some() {
        return Some(RASMError::no_tip(
            Some(ins.line),
            Some("Instruction needs rex prefix, which is forbidden in protected/compat. mode (bits 32)"),
        ));
    }
    match ins.mnem {
        Mnm::PUSH => ot_chk(
            ins,
            &[(&[R16, R32, M16, M32, I8, I16, I32, SR], Optional::Needed)],
            &[],
            &[],
        ),
        Mnm::POP => ot_chk(
            ins,
            &[(&[R16, R32, M16, M32, SR], Optional::Needed)],
            &[],
            &[],
        ),
        Mnm::MOV => ot_chk(
            ins,
            &[
                (&[R8, R16, R32, M8, M16, M32, SR, CR, DR], Optional::Needed),
                (
                    &[R8, R16, R32, M8, M16, M32, I8, I16, I32, SR, CR, DR],
                    Optional::Needed,
                ),
            ],
            &[
                (MA, MA),
                (R32, SR),
                (M32, SR),
                (M8, SR),
                (R8, SR),
                (SR, R32),
                (SR, R8),
                (SR, IA),
                (SR, M32),
                (SR, M8),
                (CR, IA),
                (CR, R8),
                (CR, R16),
                (R16, CR),
                (R8, CR),
                (CR, MA),
                (MA, CR),
                (DR, IA),
                (DR, R8),
                (DR, R16),
                (DR, R32),
                (R16, DR),
                (R8, DR),
                (DR, MA),
                (MA, DR),
                (R8, DR),
                (DR, MA),
                (MA, DR),
                (SR, CR),
                (SR, DR),
                (CR, SR),
                (CR, DR),
                (DR, SR),
                (SR, SR),
                (DR, DR),
                (CR, CR),
            ],
            &[],
        ),
        Mnm::SUB | Mnm::ADD | Mnm::CMP | Mnm::AND | Mnm::OR | Mnm::XOR => ot_chk(
            ins,
            &[
                (&[R8, R16, R32, M8, M16, M32], Optional::Needed),
                (
                    &[R8, R16, R32, M8, M16, M32, I8, I16, I32],
                    Optional::Needed,
                ),
            ],
            &[(MA, MA)],
            &[],
        ),
        Mnm::IMUL => ot_chk(
            ins,
            &[
                (&[R8, R16, R32, M8, M16, M32], Optional::Needed),
                (&[R16, R32, M16, M32], Optional::Optional),
                (&[I8, I16, I32], Optional::Optional),
            ],
            &[(MA, MA)],
            &[],
        ),
        Mnm::SAL | Mnm::SHL | Mnm::SHR | Mnm::SAR => {
            if let Some(err) = operand_check(ins, (true, true)) {
                Some(err)
            } else {
                if let Some(err) = type_check(ins.dst().unwrap(), &[R8, R16, R32, M8, M16, M32], 1)
                {
                    return Some(err);
                }
                match ins.src().unwrap() {
                    Operand::Reg(Register::CL) => None,
                    Operand::Imm(i) => {
                        if let Some(u) = i.get_uint() {
                            match Number::squeeze_u64(u) {
                                Number::UInt8(_) => None,
                                _ => Some(RASMError::with_tip(
                                    Some(ins.line),
                                    Some("Expected to found 8-bit number, found larger one instead"),
                                    Some("sal/shl/shr/sar expect 8-bit number (like 1) or cl register")
                                ))
                            }
                        } else if let Some(i) = i.get_int() {
                            match Number::squeeze_i64(i) {
                                Number::Int8(_) => None,
                                _ => Some(RASMError::with_tip(
                                    Some(ins.line),
                                    Some("Expected to found 8-bit number, found larger one instead"),
                                    Some("sal/shl/shr/sar expect 8-bit number (like 1) or cl register")
                                ))
                            }
                        } else {
                            Some(RASMError::with_tip(
                                Some(ins.line),
                                Some("Found non-compatible immediate for sal/shl/shr/sar instruction"),
                                Some("sal/shl/shr/sar only allow for 8-bit number (like 255 or -128) or cl register")
                            ))
                        }
                    }
                    _ => Some(RASMError::with_tip(
                        Some(ins.line),
                        Some(
                            "Source operand type mismatch, expected 8-bit number or cl register",
                        ),
                        Some(
                            "Consider changing source operand to 8-bit number or cl register",
                        ),
                    )),
                }
            }
        }
        Mnm::TEST => ot_chk(
            ins,
            &[
                (&[R8, R16, R32, M8, M16, M32], Optional::Needed),
                (&[I8, I16, I32, R8, R16, R32], Optional::Needed),
            ],
            &[],
            &[],
        ),
        Mnm::DIV | Mnm::IDIV | Mnm::MUL | Mnm::DEC | Mnm::INC | Mnm::NEG | Mnm::NOT => ot_chk(
            ins,
            &[(&[R8, R16, R32, M8, M16, M32], Optional::Needed)],
            &[],
            &[],
        ),
        Mnm::JMP | Mnm::CALL => ot_chk(
            ins,
            &[(&[AType::Symbol, R32, R16, M32, M16], Optional::Needed)],
            &[],
            &[],
        ),
        Mnm::JE | Mnm::JNE | Mnm::JZ | Mnm::JNZ | Mnm::JL | Mnm::JLE | Mnm::JG | Mnm::JGE => {
            ot_chk(ins, &[(&[AType::Symbol], Optional::Needed)], &[], &[])
        }
        Mnm::LEA => ot_chk(
            ins,
            &[
                (&[R16, R32], Optional::Needed),
                (&[AType::Symbol], Optional::Needed),
            ],
            &[],
            &[],
        ),
        Mnm::SYSCALL | Mnm::RET | Mnm::NOP | Mnm::POPF | Mnm::POPFD | Mnm::PUSHF | Mnm::PUSHFD => {
            ot_chk(ins, &[], &[], &[])
        }
        // #   #  #   #  #   #
        // ## ##  ## ##   # #
        // # # #  # # #    #
        // #   #  #   #   # #
        // #   #  #   #  #   #
        // (MMX/SSE2)
        Mnm::MOVD => ot_chk(
            ins,
            &[
                (&[MMX, XMM, R32, M32], Optional::Needed),
                (&[MMX, XMM, R32, M32], Optional::Needed),
            ],
            &[(M32, M32), (R32, R32), (MMX, MMX), (XMM, MMX), (MMX, XMM)],
            &[],
        ),
        Mnm::MOVQ => Some(RASMError::no_tip(
            Some(ins.line),
            Some("Instruction unsupported in 32-bit mode"),
        )),
        _ => shr_chk(ins),
    }
}

fn check_ins64bit(ins: &Instruction) -> Option<RASMError> {
    match ins.mnem {
        Mnm::CLFLUSH => ot_chk(ins, &[(&[M8], Optional::Needed)], &[], &[]),
        Mnm::PAUSE | Mnm::LFENCE | Mnm::MFENCE => ot_chk(ins, &[], &[], &[]),
        Mnm::PUSH => ot_chk(
            ins,
            &[(&[R16, R64, M16, M64, I8, I16, I32, SR], Optional::Needed)],
            &[],
            &[],
        ),
        Mnm::POP => ot_chk(
            ins,
            &[(&[R16, R64, M16, M64, SR], Optional::Needed)],
            &[],
            &[],
        ),
        Mnm::MOV => ot_chk(
            ins,
            &[
                (
                    &[R8, R16, R32, R64, M8, M16, M32, M64, SR, CR, DR],
                    Optional::Needed,
                ),
                (
                    &[
                        R8, R16, R32, R64, M8, M16, M32, M64, I8, I16, I32, I64, SR, CR, DR,
                    ],
                    Optional::Needed,
                ),
            ],
            &[
                (MA, MA),
                (R32, SR),
                (M32, SR),
                (M8, SR),
                (R8, SR),
                (SR, R32),
                (SR, R8),
                (SR, IA),
                (SR, M32),
                (SR, M8),
                (CR, IA),
                (CR, R8),
                (CR, R16),
                (CR, R32),
                (R16, CR),
                (DR, IA),
                (DR, R8),
                (DR, R16),
                (DR, R32),
                (R16, DR),
                (R8, DR),
                (DR, MA),
                (MA, DR),
                (R8, DR),
                (DR, MA),
                (MA, DR),
            ],
            &[],
        ),
        Mnm::SUB | Mnm::ADD | Mnm::CMP | Mnm::AND | Mnm::OR | Mnm::XOR => ot_chk(
            ins,
            &[
                (&[R8, R16, R32, R64, M8, M16, M32, M64], Optional::Needed),
                (
                    &[R8, R16, R32, R64, M8, M16, M32, M64, I8, I16, I32],
                    Optional::Needed,
                ),
            ],
            &[(MA, MA)],
            &[],
        ),
        Mnm::IMUL => ot_chk(
            ins,
            &[
                (&[R8, R16, R32, R64, M8, M16, M32, M64], Optional::Needed),
                (&[R16, R32, R64, M16, M32, M64], Optional::Optional),
                (&[I8, I16, I32], Optional::Optional),
            ],
            &[(MA, MA)],
            &[],
        ),
        Mnm::SAL | Mnm::SHL | Mnm::SHR | Mnm::SAR => {
            if let Some(err) = operand_check(ins, (true, true)) {
                Some(err)
            } else {
                if let Some(err) = type_check(
                    ins.dst().unwrap(),
                    &[R8, R16, R32, R64, M8, M16, M32, M64],
                    1,
                ) {
                    return Some(err);
                }
                match ins.src().unwrap() {
                    Operand::Reg(Register::CL) => None,
                    Operand::Imm(i) => {
                        if let Some(u) = i.get_uint() {
                            match Number::squeeze_u64(u) {
                                Number::UInt8(_) => None,
                                _ => Some(RASMError::with_tip(
                                    Some(ins.line),
                                    Some("expected to found 8-bit number, found larger one instead"),
                                    Some("sal/shl/shr/sar expect 8-bit number (like 1) or cl register")
                                ))
                            }
                        } else if let Some(i) = i.get_int() {
                            match Number::squeeze_i64(i) {
                                Number::Int8(_) => None,
                                _ => Some(RASMError::with_tip(
                                    Some(ins.line),
                                    Some("expected to found 8-bit number, found larger one instead"),
                                    Some("sal/shl/shr/sar expect 8-bit number (like 1) or cl register")
                                ))
                            }
                        } else {
                            Some(RASMError::with_tip(
                                Some(ins.line),
                                Some("found non-compatible immediate for sal/shl/shr/sar instruction"),
                                Some("sal/shl/shr/sar only allow for 8-bit number (like 255 or -128) or cl register")
                            ))
                        }
                    }
                    _ => Some(RASMError::with_tip(
                        Some(ins.line),
                        Some(
                            "Source operand type mismatch, expected 8-bit number or cl register"
                        ),
                        Some(
                            "Consider changing source operand to 8-bit number or cl register"
                        ),
                    )),
                }
            }
        }
        Mnm::TEST => ot_chk(
            ins,
            &[
                (&[R8, R16, R32, R64, M8, M16, M32, M64], Optional::Needed),
                (&[I8, I16, I32, R8, R16, R32, R64], Optional::Needed),
            ],
            &[],
            &[],
        ),
        Mnm::DIV | Mnm::IDIV | Mnm::MUL | Mnm::DEC | Mnm::INC | Mnm::NEG | Mnm::NOT => ot_chk(
            ins,
            &[(&[R8, R16, R32, R64, M8, M16, M32, M64], Optional::Needed)],
            &[],
            &[],
        ),
        Mnm::JMP | Mnm::CALL => ot_chk(
            ins,
            &[(&[AType::Symbol, R64, M64], Optional::Needed)],
            &[],
            &[],
        ),
        Mnm::JE | Mnm::JNE | Mnm::JZ | Mnm::JNZ | Mnm::JL | Mnm::JLE | Mnm::JG | Mnm::JGE => {
            ot_chk(ins, &[(&[AType::Symbol], Optional::Needed)], &[], &[])
        }
        Mnm::LEA => ot_chk(
            ins,
            &[
                (&[R16, R32, R64], Optional::Needed),
                (&[AType::Symbol], Optional::Needed),
            ],
            &[],
            &[],
        ),
        Mnm::SYSCALL | Mnm::RET | Mnm::NOP | Mnm::PUSHF | Mnm::POPF | Mnm::POPFQ | Mnm::PUSHFQ => {
            ot_chk(ins, &[], &[], &[])
        }
        _ => shr_chk(ins),
    }
}

pub fn shr_chk(ins: &Instruction) -> Option<RASMError> {
    match ins.mnem {
        // #####  #####  #####
        // #      #      #
        // #####  #####  #####
        //     #      #  #
        // #####  #####  #####
        // (SSE)
        Mnm::CVTSS2SI => ot_chk(
            ins,
            &[
                (&[R32, R64], Optional::Needed),
                (&[XMM, M32], Optional::Needed),
            ],
            &[],
            &[],
        ),
        Mnm::CVTPS2PI | Mnm::CVTTPS2PI => ot_chk(
            ins,
            &[(&[MMX], Optional::Needed), (&[XMM, M64], Optional::Needed)],
            &[],
            &[],
        ),
        Mnm::CVTPI2PS => ot_chk(
            ins,
            &[(&[XMM], Optional::Needed), (&[MMX, M64], Optional::Needed)],
            &[],
            &[],
        ),
        Mnm::CVTSI2SS => ot_chk(
            ins,
            &[
                (&[XMM], Optional::Needed),
                (&[XMM, R32, R64, M32, M64], Optional::Needed),
            ],
            &[],
            &[],
        ),
        Mnm::CMPSS => ot_chk(
            ins,
            &[
                (&[XMM], Optional::Needed),
                (&[XMM, M32], Optional::Needed),
                (&[I8], Optional::Needed),
            ],
            &[],
            &[],
        ),
        Mnm::UNPCKLPS | Mnm::UNPCKHPS => ot_chk(
            ins,
            &[(&[XMM], Optional::Needed), (&[XMM, M128], Optional::Needed)],
            &[],
            &[],
        ),

        Mnm::CMPPS | Mnm::SHUFPS => ot_chk(
            ins,
            &[
                (&[XMM], Optional::Needed),
                (&[XMM, M128], Optional::Needed),
                (&[I8], Optional::Needed),
            ],
            &[],
            &[],
        ),
        Mnm::MOVHPS | Mnm::MOVLPS => ot_chk(
            ins,
            &[
                (&[XMM, M64], Optional::Needed),
                (&[XMM, M64], Optional::Needed),
            ],
            &[(M64, M64)],
            &[],
        ),
        Mnm::MOVLHPS | Mnm::MOVHLPS => ot_chk(
            ins,
            &[(&[XMM], Optional::Needed), (&[XMM], Optional::Needed)],
            &[],
            &[],
        ),
        Mnm::MOVAPS | Mnm::MOVUPS => ot_chk(
            ins,
            &[
                (&[XMM, M128], Optional::Needed),
                (&[XMM, M128], Optional::Needed),
            ],
            &[(M128, M128)],
            &[],
        ),
        Mnm::MOVSS => ot_chk(
            ins,
            &[
                (&[XMM, M32], Optional::Needed),
                (&[XMM, M32], Optional::Needed),
            ],
            &[(M32, M32)],
            &[],
        ),
        Mnm::SQRTSS
        | Mnm::ADDSS
        | Mnm::SUBSS
        | Mnm::DIVSS
        | Mnm::MULSS
        | Mnm::RCPSS
        | Mnm::RSQRTSS
        | Mnm::MINSS
        | Mnm::MAXSS => ot_chk(
            ins,
            &[(&[XMM], Optional::Needed), (&[XMM, M32], Optional::Needed)],
            &[],
            &[],
        ),

        Mnm::ADDPS
        | Mnm::SUBPS
        | Mnm::DIVPS
        | Mnm::MULPS
        | Mnm::RCPPS
        | Mnm::SQRTPS
        | Mnm::RSQRTPS
        | Mnm::MINPS
        | Mnm::MAXPS
        | Mnm::ORPS
        | Mnm::ANDPS
        | Mnm::XORPS => ot_chk(
            ins,
            &[(&[XMM], Optional::Needed), (&[XMM, M128], Optional::Needed)],
            &[],
            &[],
        ),

        // #####  #####  #####   #####
        // #      #      #           #
        // #####  #####  #####   #####
        //     #      #  #       #
        // #####  #####  #####   #####
        // (SSE2)
        Mnm::MOVDQ2Q => ot_chk(
            ins,
            &[(&[MMX], Optional::Needed), (&[XMM], Optional::Needed)],
            &[],
            &[],
        ),
        Mnm::MOVMSKPD => ot_chk(
            ins,
            &[(&[R32, R64], Optional::Needed), (&[XMM], Optional::Needed)],
            &[],
            &[],
        ),
        Mnm::MOVLPD | Mnm::MOVHPD | Mnm::MOVSD => ot_chk(
            ins,
            &[
                (&[XMM, M64], Optional::Needed),
                (&[XMM, M64], Optional::Needed),
            ],
            &[(M64, M64)],
            &[],
        ),
        Mnm::MOVAPD | Mnm::MOVUPD | Mnm::MOVDQA => ot_chk(
            ins,
            &[
                (&[XMM, M128], Optional::Needed),
                (&[XMM, M128], Optional::Needed),
            ],
            &[(M128, M128)],
            &[],
        ),
        Mnm::CMPSD => ot_chk(
            ins,
            &[
                (&[XMM], Optional::Needed),
                (&[XMM, M64], Optional::Needed),
                (&[I8], Optional::Needed),
            ],
            &[],
            &[],
        ),

        Mnm::CMPPD => ot_chk(
            ins,
            &[
                (&[XMM], Optional::Needed),
                (&[XMM, M128], Optional::Needed),
                (&[I8], Optional::Needed),
            ],
            &[],
            &[],
        ),

        Mnm::SQRTSD
        | Mnm::ADDSD
        | Mnm::SUBSD
        | Mnm::DIVSD
        | Mnm::MULSD
        | Mnm::MINSD
        | Mnm::COMISD
        | Mnm::UCOMISD
        | Mnm::MAXSD => ot_chk(
            ins,
            &[(&[XMM], Optional::Needed), (&[XMM, M64], Optional::Needed)],
            &[],
            &[],
        ),

        Mnm::ADDPD
        | Mnm::SUBPD
        | Mnm::DIVPD
        | Mnm::MULPD
        | Mnm::SQRTPD
        | Mnm::MINPD
        | Mnm::MAXPD
        | Mnm::ORPD
        | Mnm::ANDPD
        | Mnm::XORPD => ot_chk(
            ins,
            &[(&[XMM], Optional::Needed), (&[XMM, M128], Optional::Needed)],
            &[],
            &[],
        ),

        Mnm::CVTPD2PI
        | Mnm::CVTTPD2PI
        | Mnm::CVTPI2PD
        | Mnm::CVTPS2DQ
        | Mnm::CVTTPS2DQ
        | Mnm::CVTDQ2PS
        | Mnm::CVTPS2PD
        | Mnm::CVTPD2PS
        | Mnm::CVTSS2SD
        | Mnm::CVTPS2SS => ot_chk(
            ins,
            &[(&[XMM], Optional::Needed), (&[XMM, M128], Optional::Needed)],
            &[],
            &[],
        ),
        Mnm::CVTDQ2PD => ot_chk(
            ins,
            &[(&[XMM], Optional::Needed), (&[XMM, M64], Optional::Needed)],
            &[],
            &[],
        ),
        Mnm::CVTSD2SI | Mnm::CVTTSD2SI => ot_chk(
            ins,
            &[
                (&[R32, R64], Optional::Needed),
                (&[XMM, M64], Optional::Needed),
            ],
            &[],
            &[],
        ),
        Mnm::CVTSI2SD => ot_chk(
            ins,
            &[
                (&[XMM], Optional::Needed),
                (&[XMM, R32, R64, M32, M64], Optional::Needed),
            ],
            &[],
            &[],
        ),

        Mnm::MASKMOVDQU => ot_chk(
            ins,
            &[(&[XMM], Optional::Needed), (&[XMM], Optional::Needed)],
            &[],
            &[],
        ),
        Mnm::MOVNTDQ | Mnm::MOVNTPD => ot_chk(
            ins,
            &[(&[M128], Optional::Needed), (&[XMM], Optional::Needed)],
            &[],
            &[],
        ),
        Mnm::MOVNTI => ot_chk(
            ins,
            &[
                (&[M32, M64], Optional::Needed),
                (&[R32, R64], Optional::Needed),
            ],
            &[],
            &[],
        ),

        // #   #  #   #  #   #
        // ## ##  ## ##   # #
        // # # #  # # #    #
        // #   #  #   #   # #
        // #   #  #   #  #   #
        // (MMX/SSE2)
        Mnm::EMMS => ot_chk(ins, &[], &[], &[]),
        Mnm::MOVD => ot_chk(
            ins,
            &[
                (&[MMX, XMM, R32, M32], Optional::Needed),
                (&[MMX, XMM, R32, M32], Optional::Needed),
            ],
            &[
                (M32, M32),
                (R32, R32),
                (MMX, MMX),
                (XMM, XMM),
                (XMM, MMX),
                (MMX, XMM),
            ],
            &[],
        ),
        Mnm::MOVQ => ot_chk(
            ins,
            &[
                (&[MMX, XMM, R64, M64], Optional::Needed),
                (&[MMX, XMM, R64, M64], Optional::Needed),
            ],
            &[
                (M64, M64),
                (R64, R64),
                (MMX, MMX),
                (XMM, XMM),
                (XMM, MMX),
                (MMX, XMM),
            ],
            &[],
        ),
        Mnm::PSLLW
        | Mnm::PSLLD
        | Mnm::PSLLQ
        | Mnm::PSRLW
        | Mnm::PSRLD
        | Mnm::PSRLQ
        | Mnm::PSRAD
        | Mnm::PSRAW => ot_chk(
            ins,
            &[
                (&[MMX, XMM], Optional::Needed),
                (&[I8, MMX, XMM, M64, M128], Optional::Needed),
            ],
            &[(XMM, MMX), (MMX, XMM), (MMX, M128), (XMM, M64)],
            &[],
        ),
        Mnm::PADDB
        | Mnm::PADDW
        | Mnm::PADDD
        | Mnm::PADDQ
        | Mnm::PADDSB
        | Mnm::PADDSW
        | Mnm::PSUBB
        | Mnm::PSUBW
        | Mnm::PSUBSB
        | Mnm::PSUBSW
        | Mnm::PMULHW
        | Mnm::PMULLW
        | Mnm::PMADDWD
        | Mnm::PCMPGTB
        | Mnm::PCMPGTW
        | Mnm::PCMPGTD
        | Mnm::PCMPEQB
        | Mnm::PCMPEQW
        | Mnm::PCMPEQD
        | Mnm::PACKSSWB
        | Mnm::PACKSSDW
        | Mnm::PACKUSWB
        | Mnm::PUNPCKLBW
        | Mnm::PUNPCKLWD
        | Mnm::PUNPCKLDQ
        | Mnm::PUNPCKHBW
        | Mnm::PUNPCKHWD
        | Mnm::PUNPCKHDQ
        | Mnm::PAND
        | Mnm::PANDN
        | Mnm::POR
        | Mnm::PXOR
        | Mnm::PSUBD => ot_chk(
            ins,
            &[
                (&[MMX, XMM], Optional::Needed),
                (&[MMX, XMM, M64, M128], Optional::Needed),
            ],
            &[(XMM, MMX), (MMX, XMM), (XMM, M64), (MMX, M128)],
            &[],
        ),
        Mnm::PMULUDQ | Mnm::PSUBQ => ot_chk(
            ins,
            &[
                (&[MMX, XMM], Optional::Needed),
                (&[MMX, M64, XMM, M128], Optional::Needed),
            ],
            &[(MMX, XMM), (XMM, MMX), (XMM, M64), (MMX, M128)],
            &[],
        ),
        Mnm::PSHUFLW | Mnm::PSHUFHW | Mnm::PSHUFD => ot_chk(
            ins,
            &[
                (&[XMM], Optional::Needed),
                (&[XMM, M128], Optional::Needed),
                (&[I8], Optional::Needed),
            ],
            &[],
            &[],
        ),
        Mnm::PSLLDQ | Mnm::PSRLDQ => ot_chk(
            ins,
            &[(&[XMM], Optional::Needed), (&[I8], Optional::Needed)],
            &[],
            &[],
        ),

        // #####  #####  #####   ####
        // #      #      #           #
        // #####  #####  #####   ####
        //     #      #  #       #
        // #####  #####  #####   ####
        // (SSE 3)
        Mnm::ADDSUBPD
        | Mnm::ADDSUBPS
        | Mnm::HADDPD
        | Mnm::HADDPS
        | Mnm::HSUBPS
        | Mnm::HSUBPD
        | Mnm::MOVSLDUP
        | Mnm::MOVSHDUP => ot_chk(
            ins,
            &[(&[XMM], Optional::Needed), (&[XMM, M128], Optional::Needed)],
            &[],
            &[],
        ),

        // weird one
        Mnm::LDDQU => ot_chk(
            ins,
            &[(&[XMM], Optional::Needed), (&[M128], Optional::Needed)],
            &[],
            &[],
        ),
        Mnm::MOVDDUP => ot_chk(
            ins,
            &[(&[XMM], Optional::Needed), (&[XMM, M64], Optional::Needed)],
            &[],
            &[],
        ),
        Mnm::MONITOR | Mnm::MWAIT | Mnm::MFENCE | Mnm::LFENCE | Mnm::CLFLUSH => {
            ot_chk(ins, &[], &[], &[])
        }

        // ##### ##### #####  #####   ####
        // #     #     #      #           #
        // ##### ##### #####  #####   ####
        //     #     #     #  #           #
        // ##### ##### #####  #####   ####
        // (SSSE 3)
        Mnm::PABSW
        | Mnm::PABSD
        | Mnm::PABSB
        | Mnm::PSIGNW
        | Mnm::PSIGND
        | Mnm::PSIGNB
        | Mnm::PHSUBW
        | Mnm::PHSUBD
        | Mnm::PHADDW
        | Mnm::PHADDD
        | Mnm::PSHUFB
        | Mnm::PHSUBSW
        | Mnm::PHADDSW
        | Mnm::PMULHRSW
        | Mnm::PMADDUBSW => ot_chk(
            ins,
            &[
                (&[MMX, XMM], Optional::Needed),
                (&[MMX, XMM, M64, M128], Optional::Needed),
            ],
            &[(MMX, XMM), (XMM, M64), (XMM, MMX), (MMX, M128)],
            &[],
        ),

        Mnm::PALIGNR => ot_chk(
            ins,
            &[
                (&[MMX, XMM], Optional::Needed),
                (&[MMX, XMM, M64, M128], Optional::Needed),
                (&[I8], Optional::Needed),
            ],
            &[(MMX, XMM), (XMM, M64), (XMM, MMX), (MMX, M128)],
            &[],
        ),

        // #####  #####  #####  #   #
        // #      #      #      #   #
        // #####  #####  #####  #####
        //     #      #  #          #
        // #####  #####  #####      #
        // (SSE4)
        Mnm::PINSRQ => ot_chk(
            ins,
            &[
                (&[XMM], Optional::Needed),
                (&[R64, M64], Optional::Needed),
                (&[I8], Optional::Needed),
            ],
            &[],
            &[],
        ),
        Mnm::PINSRD => ot_chk(
            ins,
            &[
                (&[XMM], Optional::Needed),
                (&[R32, M32], Optional::Needed),
                (&[I8], Optional::Needed),
            ],
            &[],
            &[],
        ),
        Mnm::PEXTRB => ot_chk(
            ins,
            &[
                (&[R32, R64, M32, M64], Optional::Needed),
                (&[XMM], Optional::Needed),
                (&[I8], Optional::Needed),
            ],
            &[],
            &[],
        ),
        Mnm::PEXTRD => ot_chk(
            ins,
            &[
                (&[R32, M32], Optional::Needed),
                (&[XMM], Optional::Needed),
                (&[I8], Optional::Needed),
            ],
            &[],
            &[],
        ),
        Mnm::PEXTRQ => ot_chk(
            ins,
            &[
                (&[R64, M64], Optional::Needed),
                (&[XMM], Optional::Needed),
                (&[I8], Optional::Needed),
            ],
            &[],
            &[],
        ),
        Mnm::PINSRB => ot_chk(
            ins,
            &[
                (&[XMM], Optional::Needed),
                (&[R32, M8], Optional::Needed),
                (&[I8], Optional::Needed),
            ],
            &[],
            &[],
        ),
        Mnm::PEXTRW => ot_chk(
            ins,
            &[
                (&[M16], Optional::Needed),
                (&[XMM], Optional::Needed),
                (&[I8], Optional::Needed),
            ],
            &[],
            &[],
        ),
        Mnm::PTEST
        | Mnm::PMAXSB
        | Mnm::PMAXSD
        | Mnm::PMINSD
        | Mnm::PMINSB
        | Mnm::PMINUW
        | Mnm::PMULDQ
        | Mnm::PMULLD
        | Mnm::PCMPEQQ
        | Mnm::PCMPGTQ
        | Mnm::BLENDVPS
        | Mnm::BLENDVPD
        | Mnm::PACKUSDW => ot_chk(
            ins,
            &[(&[XMM], Optional::Needed), (&[XMM, M128], Optional::Needed)],
            &[],
            &[],
        ),
        Mnm::POPCNT => ot_chk(
            ins,
            &[
                (&[R16, R32, R64], Optional::Needed),
                (&[R16, M16, R32, M32, R64, M64], Optional::Needed),
            ],
            &[
                (R16, M32),
                (R16, M64),
                (R16, R32),
                (R16, R64),
                (R32, R16),
                (R32, R64),
                (R32, M16),
                (R32, M64),
                (R64, R16),
                (R64, R32),
                (R64, M16),
                (R64, M32),
            ],
            &[],
        ),
        Mnm::MOVNTDQA => ot_chk(
            ins,
            &[(&[XMM], Optional::Needed), (&[M128], Optional::Needed)],
            &[],
            &[],
        ),
        Mnm::EXTRACTPS => ot_chk(
            ins,
            &[
                (&[R32, R64, M32], Optional::Needed),
                (&[XMM], Optional::Needed),
                (&[I8], Optional::Needed),
            ],
            &[],
            &[],
        ),
        Mnm::ROUNDSS | Mnm::ROUNDSD | Mnm::INSERTPS => ot_chk(
            ins,
            &[
                (&[XMM], Optional::Needed),
                (&[XMM, M32], Optional::Needed),
                (&[I8], Optional::Needed),
            ],
            &[],
            &[],
        ),
        Mnm::DPPS
        | Mnm::DPPD
        | Mnm::BLENDPS
        | Mnm::BLENDPD
        | Mnm::PBLENDW
        | Mnm::ROUNDPS
        | Mnm::ROUNDPD
        | Mnm::MPSADBW
        | Mnm::PCMPESTRI
        | Mnm::PCMPESTRM
        | Mnm::PCMPISTRM
        | Mnm::PCMPISTRI => ot_chk(
            ins,
            &[
                (&[XMM], Optional::Needed),
                (&[XMM, M128], Optional::Needed),
                (&[I8], Optional::Needed),
            ],
            &[],
            &[],
        ),

        _ => Some(RASMError::no_tip(
            Some(ins.line),
            Some("Tried to use currently unsupported instruction."),
        )),
    }
}

// Utils

#[derive(PartialEq)]
enum Optional {
    Needed,
    Optional,
}

fn ot_chk(
    ins: &Instruction,
    ops: &[(&[AType], Optional)],
    forb: &[(AType, AType)],
    addt: &[Mnm],
) -> Option<RASMError> {
    if let Some(err) = addt_chk(ins, addt) {
        return Some(err);
    }
    if ops.is_empty() && !ins.oprs.is_empty() {
        return Some(RASMError::no_tip(
            Some(ins.line),
            Some(
                "Instruction doesn't accept any operand, but you tried to use one anyways"
            ),
        ));
    }
    for (idx, allowed) in ops.iter().enumerate() {
        if let Some(op) = ins.oprs.get(idx) {
            if let Some(err) = type_check(op, allowed.0, idx) {
                return Some(err);
            }
        } else {
            if allowed.1 == Optional::Needed {
                return Some(RASMError::no_tip(
                    Some(ins.line),
                    Some(format!("Needed operand not found at index {}", idx)),
                ));
            } else {
                break;
            }
        }
    }
    if ops.len() == 2 {
        if let Some(err) = size_chk(ins) {
            return Some(err);
        }
    }
    if let Some(err) = forb_chk(ins, forb) {
        return Some(err);
    }
    None
}

fn forb_chk(ins: &Instruction, forb: &[(AType, AType)]) -> Option<RASMError> {
    let dst_t = if let Some(dst) = ins.dst() {
        dst.atype()
    } else {
        return None;
    };
    let src_t = if let Some(src) = ins.src() {
        src.atype()
    } else {
        return None;
    };
    for f in forb {
        if (dst_t, src_t) == *f {
            return Some(RASMError::no_tip(
                Some(ins.line),
                Some(format!(
                    "Destination and Source operand have forbidden combination: ({}, {})",
                    f.0.to_string(),
                    f.1.to_string()
                )),
            ));
        }
    }
    None
}

fn operand_check(ins: &Instruction, ops: (bool, bool)) -> Option<RASMError> {
    match (ins.dst(), ops.0) {
        (None, false) | (Some(_), true) => {}
        (Some(_), false) => {
            return Some(RASMError::with_tip(
                None,
                Some("Unexpected destination operand found: expected none, found some"),
                Some("Consider removing destination operand"),
            ))
        }
        (None, true) => {
            return Some(RASMError::with_tip(
                None,
                Some("Expected destination operand, found nothing"),
                Some("Consider adding destination operand"),
            ))
        }
    };
    match (ins.src(), ops.1) {
        (None, false) | (Some(_), true) => None,
        (Some(_), false) => Some(RASMError::with_tip(
            None,
            Some("Unexpected source operand found: expected none, found some"),
            Some("Consider removing source operand"),
        )),
        (None, true) => Some(RASMError::with_tip(
            None,
            Some("Expected source operand, found nothing"),
            Some("Consider adding source operand"),
        )),
    }
}

fn type_check(operand: &Operand, accepted: &[AType], idx: usize) -> Option<RASMError> {
    if find(accepted, operand.atype()) {
        None
    } else {
        let err = RASMError::with_tip(
            None,
            Some(format!(
                "{} operand of type {} doesn't match any of expected types: {}",
                match idx {
                    0 => "Destination".to_string(),
                    1 => "Source".to_string(),
                    _ => idx.to_string(),
                },
                operand.atype().to_string(),
                atype_arr_string(accepted)
            )),
            Some(format!(
                "Consider changing {} operand to expected type or removing instruction",
                match idx {
                    0 => "destination".to_string(),
                    1 => "source".to_string(),
                    _ => idx.to_string(),
                }
            )),
        );

        if let Operand::Imm(imm) = operand {
            match imm {
                Number::UInt64(n) => {
                    if accepted.contains(&Number::squeeze_u64(*n).atype()) {
                        return None;
                    }
                }
                Number::Int64(n) => {
                    if accepted.contains(&Number::squeeze_i64(*n).atype()) {
                        return None;
                    }
                }
                _ => {}
            }
        }
        Some(err)
    }
}
fn size_chk(ins: &Instruction) -> Option<RASMError> {
    let dst = ins.dst().unwrap();
    let src = ins.src().unwrap();

    if let Operand::CtrReg(_) = dst {
        return None;
    }
    if let Operand::CtrReg(_) = src {
        return None;
    }
    // should work (i hope so)
    match (dst.atype(), src.atype()) {
        (AType::Register(_, s0) | AType::Memory(s0) | AType::SMemory(s0), AType::Immediate(s1)) => {
            if s1 <= s0 {
                None
            } else {
                if !ins.mnem.allows_diff_size(Some(s0), Some(s1)) {
                    Some(RASMError::with_tip(
                        Some(ins.line),
                        Some(
                            "Tried to use immediate that is too large for destination operand"
                        ),
                        Some(format!("Consider changing immediate to fit inside {s0}",)),
                    ))
                } else {
                    None
                }
            }
        }
        (AType::Memory(s0) | AType::SMemory(s0), AType::Memory(s1) | AType::SMemory(s1)) => {
            if s1 == s0 {
                None
            } else {
                if !ins.mnem.allows_diff_size(Some(s0), Some(s1)) {
                    Some(RASMError::with_tip(
                        Some(ins.line),
                        Some(
                            "Tried to use operand that cannot be used for destination operand"
                        ),
                        Some(format!("Consider changing operand to be {s0}",)),
                    ))
                } else {
                    None
                }
            }
        }
        (AType::Register(g0, s0), AType::Register(g1, s1)) => {
            if s1 == s0
                || ((g0 == RPurpose::Dbg
                    || g0 == RPurpose::Ctrl
                    || g0 == RPurpose::Sgmnt
                    || g0 == RPurpose::Mmx
                    || g0 == RPurpose::F128)
                    || (g1 == RPurpose::Dbg
                        || g1 == RPurpose::Ctrl
                        || g1 == RPurpose::Sgmnt
                        || g1 == RPurpose::Mmx
                        || g1 == RPurpose::F128))
            {
                None
            } else {
                if !ins.mnem.allows_diff_size(Some(s0), Some(s1)) {
                    Some(RASMError::with_tip(
                        Some(ins.line),
                        Some(
                            "Tried to use operand that cannot be used for destination operand",
                        ),
                        Some(format!("Consider changing operand to be {s0}",)),
                    ))
                } else {
                    None
                }
            }
        }

        _ => None,
    }
}

fn addt_chk(ins: &Instruction, accpt_addt: &[Mnm]) -> Option<RASMError> {
    if let Some(addt) = &ins.addt {
        for a in addt {
            if !find_bool(accpt_addt, a) {
                return Some(RASMError::no_tip(
                    Some(ins.line),
                    Some(format!(
                        "Use of forbidden additional mnemonic: {}",
                        a.to_string()
                    )),
                ));
            }
        }
    }
    None
}

fn find_bool(addts: &[Mnm], searched: &Mnm) -> bool {
    for addt in addts {
        if searched == addt {
            return true;
        }
    }
    false
}

fn find(items: &[AType], searched: AType) -> bool {
    let (size, regprp) = match searched {
        AType::Register(prp, size) => (size, Some(prp)),
        AType::Immediate(size) => (size, None),
        AType::SMemory(size) | AType::Memory(size) => (size, None),
        AType::Symbol => (Size::Any, None),
    };
    for i in items {
        let (isize, iregprp) = match i {
            AType::Register(prp, size) => (size, Some(prp)),
            AType::Immediate(size) => (size, None),
            AType::SMemory(size) | AType::Memory(size) => (size, None),
            AType::Symbol => (&Size::Any, None),
        };
        if isize == &size && regprp.as_ref() == iregprp {
            return true;
        }
    }
    false
}
