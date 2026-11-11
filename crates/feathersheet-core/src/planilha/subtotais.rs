//! Subtotal do Excel (Dados → Estrutura de tópicos → Subtotal): numa lista
//! já ordenada pela coluna de grupo, abre uma linha de total depois de cada
//! grupo com `=SUBTOTAL(n;…)`, um total geral no fim e a estrutura de
//! tópicos de três níveis (1 = só o geral, 2 = os totais, 3 = tudo).
//!
//! Abrir as linhas é o mesmo [`Workbook::mover_intervalo`] do arrastar pela
//! borda, sobre linhas inteiras (todas as colunas usadas), de baixo para
//! cima: as fórmulas que leem as células acompanham, e tudo — mover,
//! escrever os totais e os níveis — é um Ctrl+Z só na aba.
//! "Remover todos" faz o caminho de volta.
use crate::celulas::estilo_parcial::{Campo, EstiloParcial};
use crate::celulas::number::to_js_string;
use crate::celulas::refs::{col_name, Rect, MAX_ROWS};
use crate::celulas::values::Value;
use crate::planilha::sheet::topicos::Topicos;
use crate::planilha::workbook::{Mudanca, Workbook};

/// Mover um bloco percorre as fórmulas da pasta: acima disto o Subtotal
/// levaria minutos.
const LIMITE_GRUPOS: usize = 5_000;

/// As funções da caixa "Usar função", com o número do `SUBTOTAL`.
pub const FUNCOES: [(u8, &str); 11] = [
    (9, "Soma"), (3, "Contagem"), (1, "Média"), (4, "Máx"), (5, "Mín"), (6, "Produto"),
    (2, "Contar números"), (7, "DesvPad"), (8, "DesvPadP"), (10, "Var"), (11, "VarP"),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpcoesSubtotal {
    /// "A cada alteração em": coluna absoluta.
    pub coluna_grupo: u32,
    /// Número do `SUBTOTAL` (9 = soma). Ver [`FUNCOES`].
    pub funcao: u8,
    /// "Adicionar subtotal a": colunas absolutas.
    pub colunas: Vec<u32>,
}

/// O rótulo do total, como o Excel em português: "Brasil Total", "Total Geral".
fn rotulo(funcao: u8) -> &'static str {
    match funcao {
        9 => "Total",
        2 | 3 => "Contagem",
        1 => "Média",
        4 => "Máx",
        5 => "Mín",
        6 => "Produto",
        7 | 8 => "DesvPad",
        _ => "Var",
    }
}

/// A chave do grupo: o valor mostrado, sem diferenciar maiúsculas.
fn chave(v: &Value) -> String {
    match v {
        Value::Empty => String::new(),
        Value::Number(n) => to_js_string(*n),
        Value::Text(t) => t.to_lowercase(),
        Value::Bool(b) => if *b { "VERDADEIRO".into() } else { "FALSO".into() },
        Value::Error(e) => format!("{e:?}"),
    }
}

fn texto(v: &Value) -> String {
    match v {
        Value::Text(t) => t.clone(),
        outro => chave(outro),
    }
}

fn eh_subtotal(raw: &str) -> bool {
    raw.get(..10).is_some_and(|p| p.eq_ignore_ascii_case("=SUBTOTAL("))
}

impl Workbook {
    /// As linhas de `area` (sem o cabeçalho) que têm um `=SUBTOTAL(` em
    /// alguma coluna da área.
    fn linhas_de_subtotal(&self, indice: usize, area: &Rect) -> Vec<u32> {
        let Some(aba) = self.aba(indice) else { return Vec::new() };
        let s = &aba.sheet;
        (area.r0 + 1..=area.r1)
            .filter(|&r| (area.c0..=area.c1).any(|c| eh_subtotal(&s.get_raw(r, c))))
            .collect()
    }

    /// Aplica o Subtotal na lista `area` (a primeira linha é o cabeçalho),
    /// substituindo subtotais que já existam nela.
    pub fn subtotais(&mut self, indice: usize, area: Rect, op: &OpcoesSubtotal) -> Result<Mudanca, String> {
        let area = area.normalized();
        if area.r1 <= area.r0 { return Err(crate::tr!("Selecione a lista com o cabeçalho e ao menos uma linha.").into()); }
        if !(area.c0..=area.c1).contains(&op.coluna_grupo) { return Err(crate::tr!("A coluna de grupo está fora da lista.").into()); }
        if op.colunas.is_empty() { return Err(crate::tr!("Escolha ao menos uma coluna para o subtotal.").into()); }
        if op.colunas.iter().any(|c| !(area.c0..=area.c1).contains(c)) {
            return Err(crate::tr!("Uma coluna do subtotal está fora da lista.").into());
        }
        if !FUNCOES.iter().any(|(n, _)| *n == op.funcao) { return Err(crate::tr!("Função de subtotal desconhecida.").into()); }
        self.aba(indice).ok_or("Aba inexistente.")?;

        self.aba_mut(indice).ok_or("Aba inexistente.")?.sheet.abrir_transacao();
        let resultado = self.subtotais_na_transacao(indice, area, op);
        let aba = self.aba_mut(indice).ok_or("Aba inexistente.")?;
        // Nada pela metade: com erro, o que já andou volta.
        if resultado.is_err() { aba.sheet.reverter_transacao(); } else { aba.sheet.fechar_transacao(); }
        resultado?;
        self.editar(indice, |_| {}).ok_or_else(|| "Aba inexistente.".into())
    }

    fn subtotais_na_transacao(&mut self, indice: usize, area: Rect, op: &OpcoesSubtotal) -> Result<(), String> {
        let area = self.remover_na_transacao(indice, area)?;
        let s = &self.aba(indice).ok_or("Aba inexistente.")?.sheet;
        let usado = s.used_rect();
        // A lista termina na última linha usada dela.
        let fim = (area.r0 + 1..=area.r1.min(usado.r1)).rev()
            .find(|&r| (area.c0..=area.c1).any(|c| s.has_content(r, c)))
            .ok_or_else(|| crate::tr!("A lista não tem dados abaixo do cabeçalho."))?;

        // Os grupos: sequências com a mesma chave na coluna de grupo.
        let mut grupos: Vec<(u32, u32, String)> = Vec::new();
        let mut anterior: Option<String> = None;
        for r in area.r0 + 1..=fim {
            let v = self.valor(indice, r, op.coluna_grupo);
            let k = chave(&v);
            match grupos.last_mut() {
                Some(g) if anterior.as_deref() == Some(k.as_str()) => g.1 = r,
                _ => grupos.push((r, r, texto(&v))),
            }
            anterior = Some(k);
        }
        if grupos.len() > LIMITE_GRUPOS {
            return Err(crate::trf!("São {} grupos; o limite é {LIMITE_GRUPOS}. Ordene a lista pela coluna de grupo.", grupos.len()));
        }
        let g = grupos.len() as u32;
        if usado.r1.max(fim) + g + 1 >= MAX_ROWS {
            return Err(crate::tr!("Não cabe: os totais empurrariam dados para fora da planilha.").into());
        }
        let c1 = usado.c1.max(area.c1);

        // Abrir espaço, de baixo para cima.
        if usado.r1 > fim {
            self.mover_intervalo(indice, Rect::new(fim + 1, 0, usado.r1, c1), (g + 1) as i32, 0)
                .ok_or_else(|| crate::tr!("Não consegui deslocar o que está abaixo da lista."))?;
        }
        for (k, &(a, b, _)) in grupos.iter().enumerate().skip(1).rev() {
            self.mover_intervalo(indice, Rect::new(a, 0, b, c1), k as i32, 0)
                .ok_or_else(|| crate::tr!("Não consegui abrir espaço para os totais."))?;
        }

        // Escrever os totais e os níveis.
        let nome = rotulo(op.funcao);
        let funcao = op.funcao;
        let primeira = area.r0 + 1;
        let ultima_total = fim + g;
        let geral = ultima_total + 1;
        let mut negrito = EstiloParcial::new();
        negrito.insert("bold".into(), Campo::Logico(true));
        let sheet = &mut self.aba_mut(indice).ok_or("Aba inexistente.")?.sheet;
        let mut topicos = sheet.topicos.clone();
        topicos.deslocar(false, fim + 1, g + 1, false);
        let linhas = &mut topicos.linhas;
        for (k, (a, b, rotulo_grupo)) in grupos.iter().enumerate() {
            let (a, b) = (a + k as u32, b + k as u32);
            let total = b + 1;
            sheet.set_cell(total, op.coluna_grupo, &format!("{rotulo_grupo} {nome}"));
            for &c in &op.colunas {
                let col = col_name(c);
                sheet.set_cell(total, c, &format!("=SUBTOTAL({funcao};{col}{}:{col}{})", a + 1, b + 1));
            }
            for r in a..=b { linhas.niveis.insert(r, 2); }
            linhas.niveis.insert(total, 1);
        }
        sheet.set_cell(geral, op.coluna_grupo, &format!("{nome} Geral"));
        for &c in &op.colunas {
            let col = col_name(c);
            sheet.set_cell(geral, c, &format!("=SUBTOTAL({funcao};{col}{}:{col}{})", primeira + 1, ultima_total + 1));
        }
        let rotulos = (0..g).map(|k| grupos[k as usize].1 + k + 1).chain([geral]);
        for r in rotulos {
            sheet.set_style(&Rect::new(r, area.c0, r, area.c1), &negrito);
        }
        sheet.registrar_topicos(topicos);
        Ok(())
    }

    /// "Remover todos" da janela Subtotal: tira as linhas de `=SUBTOTAL(`
    /// da lista, fecha os buracos e apaga a estrutura de tópicos dela.
    pub fn remover_subtotais(&mut self, indice: usize, area: Rect) -> Result<Mudanca, String> {
        let area = area.normalized();
        self.aba_mut(indice).ok_or("Aba inexistente.")?.sheet.abrir_transacao();
        let resultado = self.remover_na_transacao(indice, area).map(|_| ());
        let aba = self.aba_mut(indice).ok_or("Aba inexistente.")?;
        if resultado.is_err() { aba.sheet.reverter_transacao(); } else { aba.sheet.fechar_transacao(); }
        resultado?;
        self.editar(indice, |_| {}).ok_or_else(|| "Aba inexistente.".into())
    }

    /// Remove os subtotais de `area` e devolve a área encolhida.
    fn remover_na_transacao(&mut self, indice: usize, area: Rect) -> Result<Rect, String> {
        let linhas = self.linhas_de_subtotal(indice, &area);
        let s = &self.aba(indice).ok_or("Aba inexistente.")?.sheet;
        let usado = s.used_rect();
        let c1 = usado.c1.max(area.c1);
        // Sem subtotal: só os níveis da lista saem.
        let mut topicos: Topicos = s.topicos.clone();
        let eixo = &mut topicos.linhas;
        let da_lista: Vec<u32> = eixo.niveis.range(area.r0..=area.r1).map(|(&r, _)| r).collect();
        for r in &da_lista { eixo.niveis.remove(r); }
        eixo.recolhidas.retain(|r| !(area.r0..=area.r1 + 1).contains(r));

        let sheet = &mut self.aba_mut(indice).ok_or("Aba inexistente.")?.sheet;
        for &r in &linhas {
            for c in 0..=c1 {
                if sheet.has_content(r, c) { sheet.set_cell(r, c, ""); }
            }
        }
        // Reexibe o que a estrutura escondia.
        for &r in &da_lista {
            if sheet.row_heights.get(&r) == Some(&0) { sheet.registrar_dimensao(false, r, None); }
        }
        for (j, &r) in linhas.iter().enumerate() {
            let ate = linhas.get(j + 1).map_or(usado.r1, |&p| p - 1);
            if r + 1 <= ate && ate <= usado.r1 {
                self.mover_intervalo(indice, Rect::new(r + 1, 0, ate, c1), -(j as i32 + 1), 0)
                    .ok_or_else(|| crate::tr!("Não consegui fechar as linhas dos subtotais."))?;
            }
        }
        for (j, &r) in linhas.iter().enumerate().rev() {
            let _ = j;
            topicos.deslocar(false, r, 1, true);
        }
        let sheet = &mut self.aba_mut(indice).ok_or("Aba inexistente.")?.sheet;
        sheet.registrar_topicos(topicos);
        let n = linhas.len() as u32;
        Ok(Rect::new(area.r0, area.c0, area.r1.saturating_sub(n).max(area.r0), area.c1))
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn lista() -> Workbook {
        let mut w = Workbook::new();
        w.editar(0, |s| {
            for (r, (pais, v)) in [("País", "Valor"), ("Brasil", "10"), ("Brasil", "5"), ("Chile", "7"), ("Peru", "1"), ("peru", "2")]
                .iter().enumerate() {
                s.set_cell(r as u32, 0, pais);
                s.set_cell(r as u32, 1, v);
            }
            s.set_cell(8, 0, "abaixo");
            s.set_cell(9, 3, "=B2*2");
            s.clear_history();
        });
        w
    }
    fn raw(w: &Workbook, r: u32, c: u32) -> String { w.aba(0).unwrap().sheet.get_raw(r, c) }
    fn op() -> OpcoesSubtotal { OpcoesSubtotal { coluna_grupo: 0, funcao: 9, colunas: vec![1] } }

    #[test]
    fn soma_por_grupo_total_geral_e_niveis() {
        let mut w = lista();
        w.subtotais(0, Rect::new(0, 0, 5, 1), &op()).unwrap();
        assert_eq!(raw(&w, 3, 0), "Brasil Total");
        assert_eq!(w.valor(0, 3, 1), Value::Number(15.0));
        assert_eq!(raw(&w, 5, 0), "Chile Total");
        assert_eq!((raw(&w, 6, 0), raw(&w, 7, 0)), ("Peru".into(), "peru".into()), "sem diferenciar maiúsculas");
        assert_eq!(raw(&w, 8, 0), "Peru Total");
        assert_eq!(raw(&w, 9, 0), "Total Geral");
        assert_eq!(w.valor(0, 9, 1), Value::Number(25.0), "o geral não soma os totais de novo");
        assert_eq!(raw(&w, 12, 0), "abaixo", "o que estava abaixo desceu 4 linhas");
        assert_eq!(raw(&w, 13, 3), "=B2*2", "fórmula fora da lista continua lendo o mesmo dado");
        let t = &w.aba(0).unwrap().sheet.topicos.linhas;
        assert_eq!((t.nivel(1), t.nivel(3), t.nivel(9)), (2, 1, 0));
        assert!(w.aba(0).unwrap().sheet.estilo_resolvido(3, 0).is_some_and(|e| e.bold));
        // Recolhido no nível 2, o total continua (oculta à mão para o SUBTOTAL 9).
        w.editar(0, |s| s.mostrar_nivel(false, 2).unwrap());
        assert_eq!(w.aba(0).unwrap().sheet.row_heights.get(&1), Some(&0));
        assert_eq!(w.valor(0, 3, 1), Value::Number(15.0));
        w.escrever_celula(0, 3, 2, "=SUBTOTAL(109;B2:B3)".into());
        assert_eq!(w.valor(0, 3, 2), Value::Number(0.0), "o 109 ignora, e sem \"-0\"");
        w.editar(0, |s| { s.undo(); s.undo(); });
        // Um Ctrl+Z volta tudo.
        assert!(w.editar(0, |s| { s.undo(); }).is_some());
        assert_eq!(raw(&w, 3, 0), "Chile");
        assert_eq!(raw(&w, 8, 0), "abaixo");
        assert!(w.aba(0).unwrap().sheet.topicos.vazio());
    }

    #[test]
    fn de_novo_substitui_e_remover_todos_volta_a_lista() {
        let mut w = lista();
        w.subtotais(0, Rect::new(0, 0, 5, 1), &op()).unwrap();
        // De novo, com a área já crescida: substitui, não duplica.
        let contar = OpcoesSubtotal { funcao: 3, ..op() };
        w.subtotais(0, Rect::new(0, 0, 9, 1), &contar).unwrap();
        assert_eq!(raw(&w, 3, 0), "Brasil Contagem");
        assert_eq!(raw(&w, 9, 0), "Contagem Geral");
        assert_eq!(raw(&w, 12, 0), "abaixo");
        w.remover_subtotais(0, Rect::new(0, 0, 9, 1)).unwrap();
        assert_eq!((raw(&w, 3, 0), raw(&w, 5, 0), raw(&w, 6, 0)), ("Chile".into(), "peru".into(), "".into()));
        assert_eq!(raw(&w, 8, 0), "abaixo");
        assert_eq!(raw(&w, 9, 3), "=B2*2");
        assert!(w.aba(0).unwrap().sheet.topicos.vazio());
    }

    #[test]
    fn recusas() {
        let mut w = lista();
        assert!(w.subtotais(0, Rect::new(0, 0, 5, 1), &OpcoesSubtotal { colunas: vec![], ..op() }).is_err());
        assert!(w.subtotais(0, Rect::new(0, 0, 5, 1), &OpcoesSubtotal { coluna_grupo: 4, ..op() }).is_err());
        assert!(w.subtotais(0, Rect::new(0, 0, 0, 1), &op()).is_err());
        assert_eq!(raw(&w, 3, 0), "Chile", "recusa não muda nada");
    }
}
