//! `ByRef` (o padrão do VBA): o parâmetro que o procedimento altera volta para quem chamou.
//!
//! O JavaScript passa número e texto por valor. Então o parâmetro alterado vira uma caixa:
//! dentro do procedimento ele é `p.v`, e quem chama passa `{ get v() { return x; }, set v($v)
//! { x = $v; } }` quando o argumento é uma variável (a mudança volta para ela) ou `{ v: valor }`
//! quando é uma expressão (como no VBA, onde a mudança se perde). Matriz já vai por referência;
//! `Optional` e objetos ficam como estão.
use super::contexto::tipo_declarado;
use super::tipos::{Tipo, T};
use crate::sintaxe::{Expr, Instr, Proc, TipoInstr};

/// Para cada parâmetro: se vira caixa (ByRef de valor simples que o corpo altera).
pub fn caixas(proc: &Proc) -> Vec<bool> {
    proc.params
        .iter()
        .map(|p| {
            !p.por_valor && !p.matriz && !p.opcional && matches!(tipo_declarado(p.tipo.as_deref()), Tipo::Valor(t) if t != T::Matriz)
                && altera(&proc.corpo, &p.nome)
        })
        .collect()
}

/// Se algum `x = …`, `For x = …` ou `For Each x In …` do corpo muda `nome`.
fn altera(corpo: &[Instr], nome: &str) -> bool {
    let e = |n: &str| n.eq_ignore_ascii_case(nome);
    corpo.iter().any(|i| match &i.k {
        TipoInstr::Atribuir { alvo: Expr::Nome(n), set: false, .. } => e(n),
        TipoInstr::Para { var, corpo, .. } | TipoInstr::ParaCada { var, corpo, .. } => e(var) || altera(corpo, nome),
        TipoInstr::Se { ramos, senao } => ramos.iter().any(|(_, c)| altera(c, nome)) || senao.as_deref().is_some_and(|c| altera(c, nome)),
        TipoInstr::Escolha { casos, senao, .. } => casos.iter().any(|(_, c)| altera(c, nome)) || senao.as_deref().is_some_and(|c| altera(c, nome)),
        TipoInstr::Faca { corpo, .. } | TipoInstr::Com { corpo, .. } => altera(corpo, nome),
        _ => false,
    })
}

/// A caixa do argumento: a variável `js` (lida e escrita) ou um valor solto.
pub fn caixa(variavel: Option<&str>, valor: &str) -> String {
    match variavel {
        Some(x) => format!("{{ $ref: 1, get v() {{ return {x}; }}, set v($v) {{ {x} = $v; }} }}"),
        None => format!("{{ $ref: 1, v: {valor} }}"),
    }
}

/// Primeira linha do procedimento: quem não mandou caixa (o app chamando um evento, a fórmula
/// chamando a função) manda o valor solto, que vira caixa aqui.
pub fn embrulhar(js: &str) -> String {
    format!("  if (!{js}?.$ref) {js} = {{ $ref: 1, v: {js} }};")
}
