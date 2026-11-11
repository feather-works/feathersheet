//! Migração de api-valores.ts::copy para o host de macros do FeatherSheet.
//! Primeiro fotografa a ORIGEM inteira (apenas células ocupadas), depois
//! escreve o destino. Isso permite copiar sobre a própria região sem ler
//! células já modificadas. Nunca clona a planilha ou o Workbook inteiro.
use crate::enderecos;
use feathersheet_core::{
    celulas::{
        number::to_js_string,
        refs::{Rect, MAX_COLS, MAX_ROWS},
        estilo_parcial::EstiloParcial,
        values::Value,
    },
    planilha::{sheet::{edicao::Snap, Sheet}, workbook::Workbook},
};

const MAX_CELULAS_MACRO: u64 = 500_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Modo {
    Tudo,
    Valores,
    Formulas,
    Formatos,
}
impl Modo {
    pub(crate) fn ler(texto: &str) -> Result<Self, String> {
        match texto {
            "all" => Ok(Self::Tudo),
            "values" => Ok(Self::Valores),
            "formulas" => Ok(Self::Formulas),
            "formats" => Ok(Self::Formatos),
            _ => Err(feathersheet_core::trf!("Tipo de colagem inválido: {texto}. Use all, values, formulas ou formats"
            )),
        }
    }
}

/// Uma célula com alguma informação necessária; regiões vazias do plano
/// ficam implícitas e podem apagar o conteúdo/estilo do destino.
struct Celula {
    dr: u32,
    dc: u32,
    raw: String,
    estilo: Option<EstiloParcial>,
}

/// Plano é independente do Workbook após leitura. A escrita é sempre
/// feita dentro da transação aberta pelo host, preservando Ctrl+Z.
pub(crate) struct Plano {
    destino_linha: u32,
    destino_coluna: u32,
    altura: u32,
    largura: u32,
    delta_linha: i64,
    delta_coluna: i64,
    modo: Modo,
    celulas: Vec<Celula>,
}

fn valor_digitavel(valor: Value) -> String {
    match valor {
        Value::Empty => String::new(),
        Value::Text(texto) => texto,
        Value::Number(n) => to_js_string(n),
        Value::Bool(b) => if b { "VERDADEIRO" } else { "FALSO" }.into(),
        Value::Error(e) => e.code.as_str().into(),
    }
}

fn sobreposicao(a: &Rect, b: &Rect) -> Option<Rect> {
    let r0 = a.r0.max(b.r0);
    let c0 = a.c0.max(b.c0);
    let r1 = a.r1.min(b.r1);
    let c1 = a.c1.min(b.c1);
    (r0 <= r1 && c0 <= c1).then(|| Rect::new(r0, c0, r1, c1))
}

/// Pré-valida a origem, o destino e os limites antes de alterar células.
/// A posição relativa de uma fonte parcial é preservada: copiar A1:A10
/// para D1, com valor só em A8, grava em D8, não em D1.
pub(crate) fn preparar(
    wb: &Workbook,
    aba: usize,
    origem: &Rect,
    destino: &Rect,
    modo: Modo,
) -> Result<Option<Plano>, String> {
    if origem.sheet.is_some() || destino.sheet.is_some() {
        return Err(feathersheet_core::tr!("Use a API da aba para copiar; não qualifique outra aba no endereço").into());
    }
    let origem = origem.normalized();
    let destino = destino.normalized();
    if origem.r1 >= MAX_ROWS || origem.c1 >= MAX_COLS
        || destino.r0 >= MAX_ROWS || destino.c0 >= MAX_COLS {
        return Err(feathersheet_core::tr!("Intervalo fora dos limites da planilha").into());
    }
    if origem.area() > MAX_CELULAS_MACRO {
        return Err(feathersheet_core::tr!("Cópia acima de 500.000 células: reduza a origem").into());
    }
    let altura = origem.r1 - origem.r0 + 1;
    let largura = origem.c1 - origem.c0 + 1;
    let ultima_linha = destino.r0.checked_add(altura - 1)
        .ok_or_else(|| feathersheet_core::tr!("Destino ultrapassa o limite de linhas"))?;
    let ultima_coluna = destino.c0.checked_add(largura - 1)
        .ok_or_else(|| feathersheet_core::tr!("Destino ultrapassa o limite de colunas"))?;
    if ultima_linha >= MAX_ROWS || ultima_coluna >= MAX_COLS {
        return Err(feathersheet_core::tr!("Destino ultrapassa os limites de linhas ou colunas").into());
    }
    let sheet = &wb.aba(aba).ok_or_else(|| feathersheet_core::tr!("Aba de origem não encontrada"))?.sheet;
    if sheet.is_empty() {
        return Ok(None);
    }
    let extremos = sheet.used_rect();
    // O Contexto.usada() do TS toma (0,0) como início do usedRect:
    // https://src/macros/runtime.ts. Manter a mesma semântica, inclusive
    // células vazias entre o início do intervalo e a última linha ocupada.
    let usada = Rect::new(0, 0, extremos.r1, extremos.c1);
    let Some(parte_util) = sobreposicao(&origem, &usada) else {
        return Ok(None);
    };
    let delta_linha = i64::from(destino.r0) - i64::from(origem.r0);
    let delta_coluna = i64::from(destino.c0) - i64::from(origem.c0);
    // Igual ao c.usada() do TypeScript: copiar somente a interseção
    // com os dados realmente usados, sem limpar o resto do destino.
    let destino_linha_util = (i64::from(parte_util.r0) + delta_linha) as u32;
    let destino_coluna_util = (i64::from(parte_util.c0) + delta_coluna) as u32;
    let altura_util = parte_util.r1 - parte_util.r0 + 1;
    let largura_util = parte_util.c1 - parte_util.c0 + 1;
    let mut celulas = Vec::new();
    for r in parte_util.r0..=parte_util.r1 {
        for c in parte_util.c0..=parte_util.c1 {
            let raw = if modo == Modo::Valores {
                valor_digitavel(wb.valor(aba, r, c))
            } else if modo == Modo::Formatos {
                String::new()
            } else {
                sheet.get_raw(r, c)
            };
            let estilo = if matches!(modo, Modo::Tudo | Modo::Formatos) {
                sheet.estilo_mesclado(r, c).filter(|s| !s.is_empty())
            } else {
                None
            };
            if !raw.is_empty() || estilo.is_some() {
                celulas.push(Celula {
                    dr: r - parte_util.r0, dc: c - parte_util.c0,
                    raw, estilo,
                });
            }
        }
    }
    Ok(Some(Plano {
        destino_linha: destino_linha_util,
        destino_coluna: destino_coluna_util,
        altura: altura_util, largura: largura_util,
        delta_linha, delta_coluna,
        modo, celulas,
    }))
}

/// Grava após preparar() retornar Ok. Células vazias da fonte também
/// limpam as posições correspondentes quando o modo exige isso.
pub(crate) fn aplicar(sheet: &mut Sheet, plano: Plano) -> usize {
    let mut entradas = plano.celulas.into_iter().peekable();
    let mut alteradas = 0usize;
    for dr in 0..plano.altura {
        for dc in 0..plano.largura {
            let src = if entradas.peek()
                .is_some_and(|c| c.dr == dr && c.dc == dc) {
                entradas.next()
            } else {
                None
            };
            let r = plano.destino_linha + dr;
            let c = plano.destino_coluna + dc;
            let anterior = sheet.snapshot(r, c);
            let bruto = src.as_ref().map(|x| x.raw.as_str()).unwrap_or("");
            let raw = match plano.modo {
                Modo::Tudo | Modo::Formulas if bruto.starts_with('=') =>
                    enderecos::deslocar_formula(
                        bruto, plano.delta_linha, plano.delta_coluna,
                    ),
                Modo::Tudo | Modo::Formulas | Modo::Valores => bruto.to_string(),
                Modo::Formatos => anterior.as_ref()
                    .map(|a| a.raw.clone()).unwrap_or_default(),
            };
            let estilo = match plano.modo {
                Modo::Tudo | Modo::Formatos =>
                    src.and_then(|x| x.estilo),
                Modo::Valores | Modo::Formulas =>
                    anterior.and_then(|a| a.style),
            };
            let proximo = if raw.is_empty() && estilo.is_none() {
                None
            } else {
                Some(Snap { raw, style: estilo })
            };
            if sheet.apply(r, c, proximo, true) {
                alteradas += 1;
            }
        }
    }
    alteradas
}

#[cfg(test)]
mod testes {
    use super::*;
    use feathersheet_core::celulas::estilo_parcial::Campo;

    #[test]
    fn copia_com_sobreposicao_usa_origem_congelada() {
        let mut wb = Workbook::new();
        wb.escrever_celula(0,0,0,"A".into());
        wb.escrever_celula(0,1,0,"B".into());
        wb.escrever_celula(0,2,0,"C".into());
        let plano = preparar(&wb,0,&Rect::new(0,0,2,0),
            &Rect::cell(1,0),Modo::Tudo).unwrap().unwrap();
        wb.editar(0, |s| {
            s.abrir_transacao();
            assert_eq!(aplicar(s, plano), 3);
            s.fechar_transacao();
        });
        assert_eq!(wb.aba(0).unwrap().sheet.get_raw(1,0),"A");
        assert_eq!(wb.aba(0).unwrap().sheet.get_raw(2,0),"B");
        assert_eq!(wb.aba(0).unwrap().sheet.get_raw(3,0),"C");
        assert!(wb.aba_mut(0).unwrap().sheet.undo());
        assert_eq!(wb.aba(0).unwrap().sheet.get_raw(1,0),"B");
        assert_eq!(wb.aba(0).unwrap().sheet.get_raw(2,0),"C");
    }

    #[test]
    fn nao_apaga_celulas_fora_da_intersecao_usada() {
        let mut wb = Workbook::new();
        wb.escrever_celula(0,129,0,"unico".into());
        wb.escrever_celula(0,0,3,"será limpo".into());
        wb.escrever_celula(0,49,3,"preservar".into());
        let plano = preparar(&wb,0,
            &Rect::new(99,0,199,0),&Rect::cell(0,3),Modo::Tudo)
            .unwrap().unwrap();
        wb.editar(0,|s| { s.abrir_transacao(); aplicar(s,plano); s.fechar_transacao(); });
        assert_eq!(wb.aba(0).unwrap().sheet.get_raw(0,3),"");
        assert_eq!(wb.aba(0).unwrap().sheet.get_raw(49,3),"preservar");
        assert_eq!(wb.aba(0).unwrap().sheet.get_raw(30,3),"unico");
    }

    #[test]
    fn copia_formulas_ajusta_referencias_mas_valores_nao() {
        let mut wb = Workbook::new();
        wb.escrever_celula(0,0,0,"3".into());
        wb.escrever_celula(0,0,1,"=A1+2".into());
        let origem = Rect::cell(0,1);
        let dest = Rect::cell(1,1);
        let formulas = preparar(&wb,0,&origem,&dest,Modo::Formulas).unwrap().unwrap();
        wb.editar(0,|s| { s.abrir_transacao(); aplicar(s,formulas); s.fechar_transacao(); });
        assert_eq!(wb.aba(0).unwrap().sheet.get_raw(1,1),"=A2+2");
        let valores = preparar(&wb,0,&origem,&Rect::cell(2,1),Modo::Valores).unwrap().unwrap();
        wb.editar(0,|s| { s.abrir_transacao(); aplicar(s,valores); s.fechar_transacao(); });
        assert_eq!(wb.aba(0).unwrap().sheet.get_raw(2,1),"5");
    }

    #[test]
    fn copiar_so_formatos_preserva_valor_do_destino() {
        let mut wb = Workbook::new();
        wb.escrever_celula(0,0,0,"origem".into());
        wb.escrever_celula(0,0,1,"destino".into());
        let mut patch = EstiloParcial::new();
        patch.insert("bold".into(),Campo::Logico(true));
        wb.editar(0,|s| s.set_style(&Rect::cell(0,0),&patch));
        let plano=preparar(&wb,0,&Rect::cell(0,0),
            &Rect::cell(0,1),Modo::Formatos).unwrap().unwrap();
        wb.editar(0,|s| { s.abrir_transacao(); aplicar(s,plano); s.fechar_transacao(); });
        assert_eq!(wb.aba(0).unwrap().sheet.get_raw(0,1),"destino");
        assert!(wb.aba(0).unwrap().sheet.estilo_resolvido(0,1).unwrap().bold);
    }

    #[test]
    fn limite_destino_rejeitado_antes_de_alterar() {
        let mut wb = Workbook::new();
        wb.escrever_celula(0,0,0,"A".into());
        assert!(preparar(&wb,0,&Rect::new(0,0,1,1),
            &Rect::cell(MAX_ROWS-1,MAX_COLS-1),Modo::Tudo).is_err());
        assert_eq!(wb.aba(0).unwrap().sheet.get_raw(0,0),"A");
    }
}
