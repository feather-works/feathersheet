//! Formatação condicional FeatherSheet: mesmo modelo de rules.ts, sem
//! gravar as cores resultantes em células nem materializar a matriz.
use std::collections::HashMap;
use feathersheet_core::{
    celulas::{refs::{Rect, MAX_COLS, MAX_ROWS}, values::Value},
    formula::ast::Node,
    operacoes::rules::{CellFormat, ConditionalFormats, ConditionalRule, Faixa, RuleHost},
    planilha::workbook::Workbook,
};

struct Hospedeiro<'a> { pasta: &'a Workbook, aba: usize }
impl RuleHost for Hospedeiro<'_> {
    fn valor(&self, row: u32, col: u32) -> Value { self.pasta.valor(self.aba, row, col) }
    fn avaliar_em(&self, row: u32, col: u32, node: &Node) -> Value {
        self.pasta.avaliar_em(self.aba, row, col, node)
    }
    fn max_row(&self) -> u32 {
        self.pasta.aba(self.aba).map_or(0, |a| a.sheet.used_rect().r1)
    }
    fn max_col(&self) -> u32 {
        self.pasta.aba(self.aba).map_or(0, |a| a.sheet.used_rect().c1)
    }
}

struct Entrada {
    versao: u32,
    serializado: String,
    motor: ConditionalFormats,
}

/// Cache por identidade estável da aba, invalidado quando muda a versão ou a
/// lista de regras. O Canvas só pede resultados para células visíveis.
#[derive(Default)]
pub struct Estado {
    abas: HashMap<u32, Entrada>,
}

impl Estado {
    pub fn limpar(&mut self) { self.abas.clear(); }
    pub fn formato(&mut self, pasta: &Workbook, aba: usize, row: u32, col: u32) -> Option<CellFormat> {
        let viva = pasta.aba(aba)?;
        let dados = viva.sheet.r.as_deref().unwrap_or("[]");
        let entrada = self.abas.entry(viva.id).or_insert_with(|| Entrada {
            versao: viva.versao,
            serializado: String::new(),
            motor: ConditionalFormats::new(),
        });
        // Numa pintura com centenas de células não comparar a string JSON
        // inteira em TODA célula; basta quando muda a versão da aba.
        // O tamanho do JSON é conferido a cada célula por ser O(1): outra
        // pasta pode repetir o mesmo id e versão de aba (dois modelos seguidos).
        if entrada.versao != viva.versao || entrada.serializado.is_empty()
            || entrada.serializado.len() != dados.len()
        {
            if entrada.serializado != dados {
                let regras: Vec<ConditionalRule> = serde_json::from_str(dados).unwrap_or_default();
                entrada.motor.set_rules(regras);
                entrada.serializado = dados.to_owned();
            } else {
                entrada.motor.invalidate();
            }
            entrada.versao = viva.versao;
        }
        if entrada.motor.is_empty() || !entrada.motor.rules.iter().any(|regra|
            regra.ranges.iter().any(|f|
                row >= f.r0 && row <= f.r1 && col >= f.c0 && col <= f.c1
            )
        ) { return None; }
        // Fora dos ranges NÃO avalia a fórmula/célula da planilha.
        let host = Hospedeiro { pasta, aba };
        let valor = host.valor(row, col);
        entrada.motor.format_for(&host, row, col, &valor)
    }
}

/// As regras continuam armazenadas como JSON opaco da aba (`sheet.r`),
/// conforme o contrato original FSHZ/TypeScript. Nunca viram estilos de célula.
pub fn regras(pasta: &Workbook, aba: usize) -> Vec<ConditionalRule> {
    pasta.aba(aba)
        .and_then(|a| a.sheet.r.as_deref())
        .and_then(|json| serde_json::from_str(json).ok())
        .unwrap_or_default()
}

pub fn gravar(pasta: &mut Workbook, aba: usize, regras: Vec<ConditionalRule>) -> Result<(), String> {
    if pasta.aba(aba).is_none() { return Err(tr!("Aba não encontrada").into()); }
    if regras.len() > 256 || regras.iter().any(|r| r.ranges.is_empty() || r.ranges.len() > 1024
        || r.ranges.iter().any(|f| f.r0 > f.r1 || f.c0 > f.c1 ||
            f.r1 >= MAX_ROWS || f.c1 >= MAX_COLS))
    {
        return Err(tr!("Regras condicionais inválidas ou acima do limite").into());
    }
    let texto = serde_json::to_string(&regras).map_err(|e| e.to_string())?;
    pasta.editar(aba, |sheet| sheet.r = Some(texto));
    Ok(())
}

fn cruza(a: Faixa, b: Faixa) -> bool {
    a.r0 <= b.r1 && b.r0 <= a.r1 && a.c0 <= b.c1 && b.c0 <= a.c1
}

/// Equivale a `subtrair` do condicional-acoes.ts: recorta a seleção sem
/// apagar o resto da regra; até quatro retângulos, sem índices negativos.
pub fn subtrair(a: Faixa, b: Faixa) -> Vec<Faixa> {
    if !cruza(a, b) { return vec![a]; }
    let mut partes = Vec::new();
    if a.r0 < b.r0 { partes.push(Faixa {r0:a.r0,c0:a.c0,r1:b.r0-1,c1:a.c1}); }
    if a.r1 > b.r1 { partes.push(Faixa {r0:b.r1+1,c0:a.c0,r1:a.r1,c1:a.c1}); }
    let r0 = a.r0.max(b.r0);
    let r1 = a.r1.min(b.r1);
    if a.c0 < b.c0 { partes.push(Faixa {r0,c0:a.c0,r1,c1:b.c0-1}); }
    if a.c1 > b.c1 { partes.push(Faixa {r0,c0:b.c1+1,r1,c1:a.c1}); }
    partes
}

pub fn limpar_selecao(pasta: &mut Workbook, aba: usize, area: Rect) -> Result<usize, String> {
    let inicio = regras(pasta, aba);
    let selecao = Faixa {r0:area.r0,c0:area.c0,r1:area.r1,c1:area.c1};
    let mut nova = Vec::new();
    for mut regra in inicio.iter().cloned() {
        regra.ranges = regra.ranges.into_iter().flat_map(|r|subtrair(r,selecao)).collect();
        if !regra.ranges.is_empty() {nova.push(regra);}
    }
    let removidas = inicio.len() - nova.len();
    gravar(pasta, aba, nova)?;
    Ok(removidas)
}

#[cfg(test)]
mod testes {
    use super::*;
    use feathersheet_core::operacoes::rules::{Condition,CellCondition,CellOp,CondStyle};
    fn regra(bg: &str) -> ConditionalRule {
        ConditionalRule {
            id: "teste".into(),
            ranges: vec![Faixa {r0:0,c0:0,r1:3,c1:3}],
            condition: Condition::Cell(CellCondition {
                op: CellOp::GreaterThan,values:vec!["10".into()],
                style: CondStyle {bg:Some(bg.into()),..Default::default()},
            }),
        }
    }
    #[test]
    fn visual_muda_com_valor_sem_modificar_estilo_original() {
        let mut w=Workbook::new();
        w.escrever_celula(0,0,0,"15".into());
        let estilo_original = w.aba(0).unwrap().sheet.estilo_resolvido(0,0);
        gravar(&mut w,0,vec![regra("#ff0000")]).unwrap();
        let mut cache=Estado::default();
        assert_eq!(cache.formato(&w,0,0,0).unwrap().style.unwrap().bg.as_deref(),Some("#ff0000"));
        assert_eq!(w.aba(0).unwrap().sheet.estilo_resolvido(0,0),estilo_original);
        w.escrever_celula(0,0,0,"5".into());
        assert!(cache.formato(&w,0,0,0).is_none());
    }
    #[test]
    fn regras_do_typescript_roundtrip_sem_alterar_matriz() {
        let mut w=Workbook::new();
        let regras=vec![regra("#abc123")];
        gravar(&mut w,0,regras.clone()).unwrap();
        assert_eq!(super::regras(&w,0),regras);
        let bytes=feathersheet_io::da_workbook::pasta_gravar_bytes(&w,0).unwrap();
        let lida=feathersheet_io::fshz_read::ler_fshz(&bytes).unwrap();
        let nova=feathersheet_io::para_workbook::para_workbook(&lida);
        assert_eq!(super::regras(&nova,0),regras);
    }
    #[test]
    fn recorte_da_selecao_preserva_quatro_lados() {
        let a=Faixa {r0:0,c0:0,r1:9,c1:9};
        let b=Faixa {r0:3,c0:3,r1:6,c1:6};
        let partes=subtrair(a,b);
        assert_eq!(partes.len(),4);
        for f in partes { assert!(!cruza(f,b)); }
    }
    #[test]
    fn limpar_regras_da_selecao_mantem_as_demais_celulas() {
        let mut w=Workbook::new();
        gravar(&mut w,0,vec![regra("#ff0000")]).unwrap();
        limpar_selecao(&mut w,0,Rect::cell(0,0)).unwrap();
        let r=super::regras(&w,0);
        assert_eq!(r.len(),1);
        assert!(r[0].ranges.iter().all(|a| !cruza(*a,Faixa{r0:0,c0:0,r1:0,c1:0})));
    }
}
