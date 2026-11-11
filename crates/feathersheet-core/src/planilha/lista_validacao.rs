//! As opções de uma validação de lista e a conferência do valor digitado,
//! com a pasta em volta. Vale para as três formas de lista:
//!
//! - valores fixos (`"SIM,NÃO"` no Excel);
//! - intervalo, da mesma aba ou de outra (`=Dados!$A$2:$A$9`);
//! - fórmula: nome definido (`=Inversores`) ou `INDIRETO(E28)` /
//!   `INDIRETO("Inversores")` — a lista dependente do Excel, que muda
//!   conforme o que está em outra célula.
//!
//! O `INDIRETO` não existe como função do motor: aqui ele é resolvido à
//! parte, só para a lista (texto → nome ou referência → intervalo).
use crate::celulas::number::to_js_string;
use crate::celulas::refs::Rect;
use crate::celulas::values::Value;
use crate::formula::ast::Node;
use crate::operacoes::validation::{validar, Regra, ResultadoValidacao, Validacao};
use crate::planilha::workbook::Workbook;

/// Teto de opções lidas de uma origem: uma coluna inteira não trava a tela.
pub const MAXIMO_OPCOES: usize = 10_000;

fn texto(v: &Value) -> Option<String> {
    let t = match v {
        Value::Empty | Value::Error(_) => return None,
        Value::Text(s) => s.clone(),
        Value::Number(n) => to_js_string(*n),
        Value::Bool(b) => if *b { "VERDADEIRO".into() } else { "FALSO".into() },
    };
    (!t.trim().is_empty()).then_some(t)
}

impl Workbook {
    /// O índice da aba com este nome (sem diferenciar maiúsculas).
    fn indice_da_aba(&self, nome: &str) -> Option<usize> {
        (0..self.len()).find(|&i| self.aba(i).is_some_and(|a| a.sheet.name.eq_ignore_ascii_case(nome)))
    }

    /// Os textos de um intervalo, na ordem de leitura, sem vazios.
    fn textos_do_intervalo(&self, indice: usize, origem: &Rect) -> Vec<String> {
        let indice = origem.sheet.as_deref().and_then(|n| self.indice_da_aba(n)).unwrap_or(indice);
        let o = origem.normalized();
        // Coluna inteira: só até a última linha usada.
        let usado = self.aba(indice).map(|a| a.sheet.used_rect());
        let r1 = usado.as_ref().map_or(o.r1, |u| o.r1.min(u.r1));
        let c1 = usado.map_or(o.c1, |u| o.c1.min(u.c1));
        let mut saida = Vec::new();
        'fora: for r in o.r0..=r1 {
            for c in o.c0..=c1 {
                if saida.len() >= MAXIMO_OPCOES { break 'fora; }
                if let Some(t) = texto(&self.valor(indice, r, c)) { saida.push(t); }
            }
        }
        saida
    }

    /// Um texto como `Inversores`, `Dados!A2:A9` ou `$B$3:$B$9` vira o
    /// intervalo a que ele se refere (o que o `INDIRETO` faz).
    fn intervalo_do_texto(&self, texto: &str) -> Option<Rect> {
        let t = texto.trim().trim_start_matches('=');
        if t.is_empty() { return None; }
        let nome = |n: &str| self.nomeados().into_iter().find(|x| x.nome.eq_ignore_ascii_case(n)).map(|x| x.area);
        if let Some(r) = nome(t) { return Some(r); }
        // Nome com espaço não existe no Excel: "Inversores 2" vira "Inversores_2".
        if let Some(r) = nome(&t.replace(' ', "_")) { return Some(r); }
        match crate::formula::parser::parse_formula(t).ok()? {
            Node::Range { r0, c0, r1, c1, sheet } => Some(Rect { r0, c0, r1, c1, sheet }),
            Node::Ref { row, col, sheet } => Some(Rect { r0: row, c0: col, r1: row, c1: col, sheet }),
            _ => None,
        }
    }

    /// As opções da lista de `regra` para a célula (`row`, `col`) da aba
    /// `indice`. `None` quando a regra não é de lista.
    pub fn opcoes_da_lista(&self, indice: usize, row: u32, col: u32, regra: &Regra) -> Option<Vec<String>> {
        match regra {
            Regra::Lista { valores } => Some(valores.iter().filter(|s| !s.trim().is_empty())
                .take(MAXIMO_OPCOES).cloned().collect()),
            Regra::Intervalo { origem } => Some(self.textos_do_intervalo(indice, origem)),
            Regra::Formula { formula } => Some(self.opcoes_da_formula(indice, row, col, formula)),
            _ => None,
        }
    }

    fn opcoes_da_formula(&self, indice: usize, row: u32, col: u32, formula: &str) -> Vec<String> {
        let corpo = formula.trim().trim_start_matches('=').trim();
        // INDIRETO(x): x é avaliado na célula e o texto vira o intervalo.
        let maiusculo = corpo.to_uppercase();
        for prefixo in ["INDIRETO(", "INDIRECT("] {
            if maiusculo.starts_with(prefixo) && corpo.ends_with(')') {
                let dentro = &corpo[prefixo.len()..corpo.len() - 1];
                let Ok(arg) = crate::formula::parser::parse_formula(dentro) else { return Vec::new() };
                let alvo = match self.avaliar_em(indice, row, col, &arg) {
                    Value::Text(t) => t,
                    outro => texto(&outro).unwrap_or_default(),
                };
                return self.intervalo_do_texto(&alvo)
                    .map(|r| self.textos_do_intervalo(indice, &r)).unwrap_or_default();
            }
        }
        if let Some(r) = self.intervalo_do_texto(corpo) {
            return self.textos_do_intervalo(indice, &r);
        }
        // Outra fórmula: o valor dela, se der um só.
        let Ok(no) = crate::formula::parser::parse_formula(corpo) else { return Vec::new() };
        texto(&self.avaliar_em(indice, row, col, &no)).into_iter().collect()
    }

    /// Confere `valor` contra `regra` com a pasta em volta: as listas leem
    /// outra aba, nomes e `INDIRETO`; o resto é o [`validar`] de sempre.
    pub fn validar_na_pasta(&self, indice: usize, row: u32, col: u32, valor: &Value, regra: &Regra) -> ResultadoValidacao {
        let digitado = texto(valor).map(|t| t.trim().to_string());
        match (regra, digitado) {
            (_, None) => ResultadoValidacao { ok: true, motivo: None },
            (Regra::Intervalo { .. } | Regra::Formula { .. }, Some(t)) => {
                let opcoes = self.opcoes_da_lista(indice, row, col, regra).unwrap_or_default();
                if opcoes.iter().any(|o| o.trim() == t) {
                    ResultadoValidacao { ok: true, motivo: None }
                } else {
                    ResultadoValidacao { ok: false, motivo: Some(crate::trf!("O valor \"{t}\" não está presente na lista de opções.")) }
                }
            }
            _ => validar(valor, regra, Some(|r, c| self.valor(indice, r, c))),
        }
    }

    /// A regra que vale na célula (a primeira que a cobre) entre as `regras`
    /// da aba — a GUI usa para mostrar a mensagem de entrada ao selecionar e
    /// o alerta ao digitar. (O núcleo não lê JSON: quem chama decodifica o
    /// `Sheet.va`, por exemplo com `carregar_regras` da GUI.)
    pub fn validacao_da_celula(regras: &[Validacao], row: u32, col: u32) -> Option<&Validacao> {
        regras.iter().find(|v| v.ranges.iter().any(|r| r.contains(row, col)))
    }

    /// "Circular dados inválidos": as células da aba que têm valor e violam a
    /// regra que as cobre, em ordem de linha e coluna. Célula vazia nunca é
    /// inválida (como no Excel, que ignora em branco).
    pub fn circular_dados_invalidos(&self, indice: usize, regras: &[Validacao]) -> Vec<(u32, u32)> {
        let Some(aba) = self.aba(indice) else { return Vec::new() };
        let mut vistas = std::collections::BTreeSet::new();
        let mut invalidas = Vec::new();
        for regra in regras {
            for rect in &regra.ranges {
                let rect = rect.normalized();
                for chave in aba.sheet.chaves_no_retangulo(&rect) {
                    let par = crate::planilha::sheet::Sheet::key_to_coord(chave);
                    // A primeira regra que cobre a célula é a que vale.
                    if !vistas.insert(par) {
                        continue;
                    }
                    let (r, c) = par;
                    let primeira = Self::validacao_da_celula(regras, r, c);
                    if !primeira.is_some_and(|p| std::ptr::eq(p, regra)) {
                        continue;
                    }
                    let valor = self.valor(indice, r, c);
                    if !self.validar_na_pasta(indice, r, c, &valor, &regra.regra).ok {
                        invalidas.push(par);
                    }
                }
            }
        }
        invalidas.sort_unstable();
        invalidas
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn pasta() -> Workbook {
        let mut w = Workbook::new();
        w.criar_aba("Dados");
        for (r, t) in ["Sungrow", "Growatt", "Fronius"].iter().enumerate() {
            w.escrever_celula(1, r as u32, 0, (*t).into());
        }
        w.definir_nome("Inversores", Rect { r0: 0, c0: 0, r1: 2, c1: 0, sheet: Some("Dados".into()) });
        w.escrever_celula(0, 0, 4, "Inversores".into());
        w
    }

    #[test]
    fn lista_por_nome_outra_aba_e_indireto() {
        let w = pasta();
        let nome = Regra::Formula { formula: "=Inversores".into() };
        assert_eq!(w.opcoes_da_lista(0, 5, 5, &nome).unwrap(), ["Sungrow", "Growatt", "Fronius"]);
        let outra = Regra::Intervalo { origem: Rect { r0: 0, c0: 0, r1: 1_048_575, c1: 0, sheet: Some("Dados".into()) } };
        assert_eq!(w.opcoes_da_lista(0, 5, 5, &outra).unwrap().len(), 3, "coluna inteira para no usado");
        // INDIRETO(E1): E1 tem o nome da lista.
        let dependente = Regra::Formula { formula: "=INDIRECT(E1)".into() };
        assert_eq!(w.opcoes_da_lista(0, 1, 4, &dependente).unwrap().len(), 3);
        let literal = Regra::Formula { formula: "=INDIRETO(\"Inversores\")".into() };
        assert!(w.validar_na_pasta(0, 1, 4, &Value::Text("Growatt".into()), &literal).ok);
        assert!(!w.validar_na_pasta(0, 1, 4, &Value::Text("Outro".into()), &literal).ok);
        assert!(w.validar_na_pasta(0, 1, 4, &Value::Empty, &literal).ok);
    }

    #[test]
    fn numero_inteiro_e_operadores_novos() {
        use crate::operacoes::validation::NumeroOp;
        let w = Workbook::new();
        let inteiro = Regra::Numero { op: NumeroOp::MaiorIgual, a: 1.0, b: None, inteiro: true };
        assert!(w.validar_na_pasta(0, 0, 0, &Value::Text("3".into()), &inteiro).ok);
        assert!(!w.validar_na_pasta(0, 0, 0, &Value::Text("2,5".into()), &inteiro).ok);
        let fora = Regra::Numero { op: NumeroOp::Fora, a: 1.0, b: Some(5.0), inteiro: false };
        assert!(!w.validar_na_pasta(0, 0, 0, &Value::Text("3".into()), &fora).ok);
        assert!(w.validar_na_pasta(0, 0, 0, &Value::Text("9".into()), &fora).ok);
    }

    #[test]
    fn circular_dados_invalidos_e_regra_da_celula() {
        use crate::operacoes::validation::{NumeroOp, QuandoValidacao, Validacao};
        let mut w = Workbook::new();
        for (r, t) in ["10", "5", "abc", "99", ""].iter().enumerate() {
            w.escrever_celula(0, r as u32, 0, (*t).into());
        }
        w.escrever_celula(0, 0, 1, "07:30".into());
        w.escrever_celula(0, 1, 1, "23:59".into());
        let regras = vec![
            Validacao {
                ranges: vec![Rect { r0: 0, c0: 0, r1: 9, c1: 0, sheet: None }],
                regra: Regra::Numero { op: NumeroOp::Entre, a: 1.0, b: Some(50.0), inteiro: false },
                quando: QuandoValidacao::Avisar, mensagem: None, mensagens: Default::default(),
            },
            Validacao {
                ranges: vec![Rect { r0: 0, c0: 1, r1: 9, c1: 1, sheet: None }],
                regra: Regra::Hora { op: NumeroOp::Entre, a: "08:00".into(), b: Some("18:00".into()) },
                quando: QuandoValidacao::Recusar, mensagem: None,
                mensagens: crate::operacoes::validation::MensagensValidacao {
                    entrada_titulo: Some("Hora".into()), ..Default::default() },
            },
        ];
        assert_eq!(w.circular_dados_invalidos(0, &regras), vec![(0, 1), (1, 1), (2, 0), (3, 0)]);
        assert!(Workbook::validacao_da_celula(&regras, 0, 1).unwrap().mensagem_de_entrada().is_some());
        assert!(Workbook::validacao_da_celula(&regras, 5, 5).is_none());
    }
}
