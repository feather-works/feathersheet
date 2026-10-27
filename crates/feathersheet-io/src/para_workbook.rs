//! Do arquivo direto para a memória do Rust, sem passar pelo JavaScript.
//!
//! O `fshz_read.rs` já lia o `.fshz` inteiro em Rust — descomprimia, desmontava
//! o Arrow, separava célula avulsa de coluna. E então **serializava tudo para
//! o JavaScript montar a planilha**. Era por isso que abrir um arquivo de dois
//! milhões de linhas continuava enchendo o WebKit de gigabytes, mesmo com a
//! leitura em Rust: quem terminava de montar era o outro lado.
//!
//! Este módulo fecha esse caminho. A [`PastaLida`] vira uma
//! [`Workbook`](feathersheet_core::planilha::workbook::Workbook) aqui, e o que atravessa a
//! ponte depois é só o quadro do que está na tela.
//!
//! ## O que ainda se perde, e é honesto dizer
//!
//! - **Gráfico, regra de formatação condicional, tabela, consulta e tabela
//!   dinâmica** continuam como JSON opaco. O motor em Rust ainda não tem par
//!   para eles, e inventar um agora seria adivinhar o formato. Eles voltam
//!   para o TypeScript como hoje.
//! - **A coluna de fórmula empacotada** entra direto na
//!   [`FormulaTable`](feathersheet_core::formula::formula_table::FormulaTable), com o padrão e
//!   o valor já calculado — 12 bytes por célula, nada a recalcular ao abrir.
//!   É o `loadColumn` do TypeScript. Até 22/09/2026 ela entrava célula a
//!   célula pelo texto, e com dois defeitos: o `slot` era lido sem o `>> 8`
//!   (cada coluna pegava o padrão de outra) e o valor do arquivo era jogado
//!   fora. Ver os testes `cada_coluna_de_formula_pega_o_seu_padrao` e
//!   `a_formula_abre_com_o_valor_do_arquivo_sem_recalcular`.

use feathersheet_core::celulas::style::{Align, NumberFormat, Style, VAlign};
use feathersheet_core::planilha::workbook::Workbook;

use crate::fshz_read::{AbaLida, ColunaFormulaLida, ColunaNumLida, ColunaTxtLida, PastaLida};

/// Monta a pasta viva a partir do que foi lido do arquivo.
///
/// A ordem de carga importa e é a mesma do `carregarV2` do TypeScript: as
/// células **avulsas entram antes** das colunas. Cada avulsa invalida quem
/// depende dela, e carregá-las depois sujaria as fórmulas das mesmas linhas —
/// jogando fora o valor que acabou de vir do arquivo. O teste "abrir um FSH2
/// não recalcula fórmula nenhuma" do lado TypeScript existe por causa disso.
pub fn para_workbook(lida: &PastaLida) -> Workbook {
    montar(lida, None)
}

/// [`para_workbook`] soltando cada coluna lida assim que ela entra na aba:
/// sem isto, a pasta lida (4,4 GB no `2026_03.fshz`) e a montada conviviam
/// inteiras até o fim. É o caminho do abrir do aplicativo (17/10/2026).
pub fn para_workbook_soltando(mut lida: PastaLida) -> Workbook {
    let colunas = lida.sheets.iter_mut().map(|a| (
        std::mem::take(&mut a.colunas_num), std::mem::take(&mut a.colunas_txt), std::mem::take(&mut a.colunas_formula),
    )).collect();
    montar(&lida, Some(colunas))
}

type ColunasDaAba = (Vec<ColunaNumLida>, Vec<ColunaTxtLida>, Vec<ColunaFormulaLida>);

fn montar(lida: &PastaLida, colunas: Option<Vec<ColunasDaAba>>) -> Workbook {
    // A barra da montagem anda por célula carregada.
    let tamanho = |num: &[ColunaNumLida], txt: &[ColunaTxtLida], fx: &[ColunaFormulaLida]| {
        num.iter().map(|c| c.values.len() as u64).sum::<u64>()
            + txt.iter().map(|c| c.indices.len() as u64).sum::<u64>()
            + fx.iter().map(|c| c.slots.len() as u64).sum::<u64>()
    };
    crate::progresso::etapa_total(lida.sheets.iter().enumerate().map(|(i, a)| {
        a.avulsas.len() as u64 + match colunas.as_ref().and_then(|c| c.get(i)) {
            Some((n, t, f)) => tamanho(n, t, f),
            None => tamanho(&a.colunas_num, &a.colunas_txt, &a.colunas_formula),
        }
    }).sum());
    let mut pasta = Workbook::vazia();
    let mut proprias = colunas.map(|c| c.into_iter());
    for aba in &lida.sheets {
        let indice = pasta.criar_aba(aba.n.clone());
        let desta = proprias.as_mut().and_then(Iterator::next);
        if let Some(viva) = pasta.aba_mut(indice) {
            carregar_aba(&mut viva.sheet, aba, desta);
        }
    }
    // Um arquivo sem aba nenhuma não existe na prática, mas um `.fshz`
    // corrompido pode chegar assim — e uma pasta sem aba derruba tudo que vem
    // depois.
    if pasta.is_empty() {
        pasta.criar_aba("Planilha1");
    }
    // Arquivos antigos ou corrompidos podem marcar todas as abas como ocultas.
    // A interface precisa conservar pelo menos uma guia acessível.
    let sem_visiveis = pasta.abas_visiveis().next().is_none();
    if sem_visiveis {
        if let Some(aba) = pasta.aba_mut(0) {
            aba.sheet.oculta = false;
        }
    }
    if let Some(nomes) = &lida.names {
        nomes_de_json(&mut pasta, nomes);
    }
    pasta.estilos_celula = lida.es.as_ref().map(|v| v.to_string());
    if let Some(pr) = &lida.pr {
        pasta.propriedades = propriedades_de_json(pr);
    }
    pasta.macros = lida.mc.as_ref().map(|v| v.to_string());
    pasta.formularios = lida.fm.as_ref().map(|v| v.to_string());
    // Os últimos valores das pastas externas: valem até a origem abrir.
    if let Some(ex) = &lida.ex {
        pasta.instalar_externos_guardados(crate::externos_guardados::de_json(ex));
    }
    pasta
}

/// Os intervalos nomeados como o TypeScript os grava (`listNames()`):
/// `[{ name, rect: { r0, c0, r1, c1, sheet? } }]`. Entrada estranha é
/// ignorada — um nome a menos não pode impedir o arquivo de abrir.
pub fn nomes_de_json(pasta: &mut Workbook, nomes: &serde_json::Value) {
    for n in nomes.as_array().into_iter().flatten() {
        let Some(nome) = n.get("name").and_then(|v| v.as_str()) else { continue };
        let Some(r) = n.get("rect") else { continue };
        let lado = |k: &str| r.get(k).and_then(|v| v.as_f64()).map(|x| x.max(0.0) as u32);
        let (Some(r0), Some(c0), Some(r1), Some(c1)) = (lado("r0"), lado("c0"), lado("r1"), lado("c1")) else {
            continue;
        };
        let mut area = feathersheet_core::celulas::refs::Rect::new(r0, c0, r1, c1);
        area.sheet = r.get("sheet").and_then(|v| v.as_str()).map(str::to_string);
        pasta.definir_nome(nome, area);
    }
}

/// Propriedades do arquivo como vão no `.fshz` (`pr`); `None` se vazias,
/// para o arquivo sem autor não ganhar o campo.
pub fn propriedades_para_json(p: &feathersheet_core::planilha::workbook::Propriedades) -> Option<serde_json::Value> {
    (!p.vazia()).then(|| {
        let mut v = serde_json::json!({
            "autor": p.autor, "modificadoPor": p.modificado_por, "organizacao": p.organizacao,
        });
        // A proteção da estrutura vai junto: no Excel ela também é da pasta (workbook.xml).
        if let Some(pp) = &p.protecao_estrutura {
            v["protecaoEstrutura"] = serde_json::json!({ "senha": pp.senha.as_ref().map(senha_para_json) });
        }
        v
    })
}

/// A senha no formato da proteção da aba (`alg`, `hash`, `sal`, `giros`).
pub fn senha_para_json(s: &feathersheet_core::planilha::sheet::protecao::SenhaPlanilha) -> serde_json::Value {
    serde_json::json!({ "alg": s.algoritmo, "hash": s.hash, "sal": s.sal, "giros": s.giros })
}

fn senha_de_json(s: &serde_json::Value) -> Option<feathersheet_core::planilha::sheet::protecao::SenhaPlanilha> {
    Some(feathersheet_core::planilha::sheet::protecao::SenhaPlanilha {
        algoritmo: s.get("alg")?.as_str()?.to_string(),
        hash: s.get("hash")?.as_str()?.to_string(),
        sal: s.get("sal")?.as_str()?.to_string(),
        giros: u32::try_from(s.get("giros")?.as_u64()?).ok()?,
    })
}

/// O contrário de [`propriedades_para_json`]; campo ausente ou estranho fica vazio.
pub fn propriedades_de_json(v: &serde_json::Value) -> feathersheet_core::planilha::workbook::Propriedades {
    let campo = |nome: &str| v[nome].as_str().unwrap_or_default().to_owned();
    feathersheet_core::planilha::workbook::Propriedades {
        autor: campo("autor"), modificado_por: campo("modificadoPor"), organizacao: campo("organizacao"),
        protecao_estrutura: v.get("protecaoEstrutura").filter(|x| x.is_object()).map(|x|
            feathersheet_core::planilha::workbook::ProtecaoPasta { senha: x.get("senha").and_then(senha_de_json) }),
    }
}

/// O contrário: os nomes vivos, no mesmo formato.
pub fn nomes_para_json(pasta: &Workbook) -> serde_json::Value {
    serde_json::Value::Array(
        pasta
            .nomeados()
            .into_iter()
            .map(|n| {
                let a = &n.area;
                let mut rect = serde_json::json!({ "r0": a.r0, "c0": a.c0, "r1": a.r1, "c1": a.c1 });
                if let Some(s) = &a.sheet {
                    rect["sheet"] = serde_json::json!(s);
                }
                serde_json::json!({ "name": n.nome, "rect": rect })
            })
            .collect(),
    )
}

fn campo_de_json(v: &serde_json::Value) -> Option<feathersheet_core::celulas::estilo_parcial::Campo> {
    use feathersheet_core::celulas::estilo_parcial::Campo;
    Some(match v {
        serde_json::Value::Null => Campo::Nulo,
        serde_json::Value::Bool(b) => Campo::Logico(*b),
        serde_json::Value::Number(n) => Campo::Numero(n.as_f64()?),
        serde_json::Value::String(s) => Campo::Texto(s.clone()),
        serde_json::Value::Object(o) => Campo::Objeto(
            o.iter().filter_map(|(k, v)| Some((k.clone(), campo_de_json(v)?))).collect(),
        ),
        serde_json::Value::Array(_) => return None,
    })
}

pub fn estilo_parcial_de_json(v: &serde_json::Value) -> Option<feathersheet_core::celulas::estilo_parcial::EstiloParcial> {
    let obj = v.as_object()?;
    let mut e = feathersheet_core::celulas::estilo_parcial::EstiloParcial::new();
    for (k, v) in obj {
        if let Some(c) = campo_de_json(v) {
            e.insert(k.clone(), c);
        }
    }
    Some(e)
}

/// O contrário de [`crate::da_workbook::topicos_para_json`].
pub fn topicos_de_json(v: &serde_json::Value) -> feathersheet_core::planilha::sheet::topicos::Topicos {
    use feathersheet_core::planilha::sheet::topicos::{EixoTopicos, Topicos, NIVEL_MAXIMO};
    let eixo = |niveis: &str, recolhidas: &str| EixoTopicos {
        niveis: v[niveis].as_array().into_iter().flatten().filter_map(|p| {
            let i = u32::try_from(p[0].as_u64()?).ok()?;
            let n = u8::try_from(p[1].as_u64()?).ok()?.min(NIVEL_MAXIMO);
            (n > 0).then_some((i, n))
        }).collect(),
        recolhidas: v[recolhidas].as_array().into_iter().flatten()
            .filter_map(|i| u32::try_from(i.as_u64()?).ok()).collect(),
    };
    Topicos { linhas: eixo("l", "rl"), colunas: eixo("c", "rc") }
}

/// O contrário de [`crate::da_workbook::protecao_para_json`]. Permissão
/// desconhecida é ignorada; senha sem os campos todos invalida a proteção
/// inteira não: a aba fica protegida sem senha.
pub fn protecao_de_json(v: &serde_json::Value) -> Option<feathersheet_core::planilha::sheet::protecao::ProtecaoPlanilha> {
    use feathersheet_core::planilha::sheet::protecao::{Permissao, ProtecaoPlanilha};
    if !v.is_object() { return None; }
    let senha = v.get("senha").and_then(senha_de_json);
    let permitidas = v.get("p").and_then(|p| p.as_array()).into_iter().flatten()
        .filter_map(|x| Permissao::da_chave(x.as_str()?)).collect();
    Some(ProtecaoPlanilha { senha, permitidas })
}

/// O contrário de [`crate::da_workbook::comentarios_para_json`]. Item sem
/// linha ou coluna é ignorado.
pub fn comentarios_de_json(v: &serde_json::Value) -> feathersheet_core::planilha::sheet::comentarios::Comentarios {
    use feathersheet_core::planilha::sheet::comentarios::Comentario;
    let texto = |item: &serde_json::Value, k: &str| item.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
    v.as_array().into_iter().flatten().filter_map(|item| {
        let r = u32::try_from(item.get("r")?.as_u64()?).ok()?;
        let c = u32::try_from(item.get("c")?.as_u64()?).ok()?;
        let visivel = item.get("v").and_then(|x| x.as_bool()).unwrap_or(false);
        Some(((r, c), Comentario { autor: texto(item, "a"), texto: texto(item, "t"), visivel }))
    }).collect()
}

fn carregar_aba(sheet: &mut feathersheet_core::planilha::sheet::Sheet, aba: &AbaLida, proprias: Option<ColunasDaAba>) {
    sheet.tab_color = aba.tc.clone();
    sheet.oculta = aba.oc;
    sheet.largura_padrao = aba.wp;
    if let Some(tp) = &aba.tp { sheet.topicos = topicos_de_json(tp); }
    sheet.protecao = aba.pt.as_ref().and_then(protecao_de_json);
    sheet.comentarios = aba.cm.as_ref().map(comentarios_de_json).unwrap_or_default();
    sheet.linhas_ocultas_desde = aba.lo;
    sheet.colunas_ocultas_desde = aba.co;
    sheet.g = aba.g.as_ref().map(|v| v.to_string());
    sheet.pr = aba.pr.as_ref().map(|v| v.to_string());
    sheet.r = aba.r.as_ref().map(|v| v.to_string());
    sheet.tb = aba.tb.as_ref().map(|v| v.to_string());
    sheet.oq = aba.oq.as_ref().map(|v| v.to_string());
    sheet.dn = aba.dn.as_ref().map(|v| v.to_string());
    sheet.mc = aba.mc.as_ref().map(|v| v.to_string());
    sheet.im = aba.im.as_ref().map(|v| v.to_string());
    crate::pendentes::carregar_na_aba(sheet, aba.pd.as_ref());
    sheet.va = aba.va.as_ref().map(|v| v.to_string());
    sheet.bt = aba.bt.as_ref().map(|v| v.to_string());
    sheet.tn = aba.tn.as_ref().map(|v| v.to_string());
    sheet.ob = aba.ob.as_ref().map(|v| v.to_string());
    sheet.ce = aba.ce.as_ref().map(|v| v.to_string());
    sheet.col_styles = aba.estilos_coluna.iter().filter_map(|(c, v)| estilo_parcial_de_json(v).map(|e| (*c, e))).collect();
    sheet.row_styles = aba.estilos_linha.iter().filter_map(|(r, v)| estilo_parcial_de_json(v).map(|e| (*r, e))).collect();
    sheet.sheet_style = aba.estilo_aba.as_ref().and_then(estilo_parcial_de_json);

    // Cada estilo da tabela do arquivo vira `EstiloParcial` e é registrado
    // na aba uma vez só, na primeira célula que o usa.
    let mut registrados: Vec<Option<Option<usize>>> = vec![None; aba.estilos.len()];
    let mut estilo_idx = |sheet: &mut feathersheet_core::planilha::sheet::Sheet, i: usize| -> Option<usize> {
        let guardado = registrados.get_mut(i)?;
        *guardado.get_or_insert_with(|| {
            aba.estilos.get(i).and_then(estilo_parcial_de_json).and_then(|e| sheet.registrar_parcial(e))
        })
    };

    // 0) a tabela de padrões do arquivo antes das avulsas: as colunas
    //    empacotadas apontam para esses números, e uma avulsa que registrasse
    //    o seu padrão antes tomaria o número 0 de outra coluna (#45).
    if !aba.padroes_formula.is_empty() {
        sheet.formulas.load_patterns(&aba.padroes_formula);
    }

    // 1) as avulsas primeiro. Ver a nota de ordem acima.
    use feathersheet_core::planilha::sheet::carga::{CelulaPreparada, Padroes};
    let mut padroes = Padroes::default();
    let mut sobras = Vec::new();
    for (i, celula) in aba.avulsas.iter().enumerate() {
        if i % 65_536 == 65_535 { crate::progresso::etapa_somar(65_536); }
        let row = (celula.k / 16384) as u32;
        let col = (celula.k % 16384) as u32;
        let idx = celula.estilo.and_then(|i| estilo_idx(sheet, i));
        if celula.v.trim().starts_with('=') {
            // Arquivos anteriores guardavam cada PROCX como avulsa. A carga
            // decide uma vez por padrão e evita a invalidação de edição a
            // cada uma das milhões de fórmulas que agora viram coluna.
            let pronta = CelulaPreparada::preparar(row, col, &celula.v, (0, celula.v.len() as u32), 0, None, &mut padroes);
            let estilo = idx.and_then(|i| sheet.style_parts.get(i)).cloned();
            sheet.carregar_preparada(pronta, &celula.v, idx, estilo.as_ref(), &mut sobras);
            sobras.clear();
        } else {
            sheet.set_cell_raw(row, col, celula.v.clone(), idx);
        }
    }
    sheet.terminar_carga();

    // Com as colunas próprias, cada uma é solta logo depois de carregada
    // (fim da volta do `for`); emprestadas, ficam com quem chamou.
    match proprias {
        Some((num, txt, fx)) => carregar_colunas(sheet, num, txt, fx),
        None => carregar_colunas(sheet, &aba.colunas_num, &aba.colunas_txt, &aba.colunas_formula),
    }

    // 5) o estilo direto: por faixa (arquivos de 16/10/2026 em diante) e por
    //    célula (arquivos anteriores — cada célula entra na faixa da coluna
    //    e as vizinhas iguais se fundem).
    for &(col, r0, r1, i) in &aba.estilos_faixa {
        let Some(idx) = estilo_idx(sheet, i) else { continue };
        sheet.styles.inserir_faixa(col, r0, r1, idx);
    }
    for &(chave, i) in &aba.estilos_diretos {
        let Some(idx) = estilo_idx(sheet, i) else { continue };
        sheet.set_style_idx((chave / 16384) as u32, (chave % 16384) as u32, idx);
    }

    // 6) larguras e alturas. A altura zero é o que o filtro usa para esconder
    //    linha, então ela precisa sobreviver — já se perdeu uma vez aqui.
    for (col, largura) in &aba.w {
        sheet.col_widths.insert(*col, *largura as u32);
    }
    for (row, altura) in &aba.h {
        sheet.row_heights.insert(*row, *altura as u32);
    }
}

/// Passos 2 a 4 do [`carregar_aba`]: números, textos e fórmulas empacotadas.
fn carregar_colunas<N, T, F>(sheet: &mut feathersheet_core::planilha::sheet::Sheet,
    num: impl IntoIterator<Item = N>, txt: impl IntoIterator<Item = T>, fx: impl IntoIterator<Item = F>)
where N: std::borrow::Borrow<ColunaNumLida>, T: std::borrow::Borrow<ColunaTxtLida>, F: std::borrow::Borrow<ColunaFormulaLida> {
    // 2) as colunas de número, inteiras de uma vez (eram 6 milhões de
    //    `set_cell_valor` na Planilha Pesada).
    for coluna in num {
        let coluna = coluna.borrow();
        sheet.carregar_coluna_numeros(coluna.col, &coluna.values, &coluna.bitmap);
        crate::progresso::etapa_somar(coluna.values.len() as u64);
    }

    // 3) as colunas de texto, que vêm por dicionário: o dicionário entra no
    //    pool uma vez por texto distinto, os ids vão em bloco.
    for coluna in txt {
        let coluna = coluna.borrow();
        sheet.carregar_coluna_textos(coluna.col, &coluna.indices, &coluna.bitmap, &coluna.dict);
        crate::progresso::etapa_somar(coluna.indices.len() as u64);
    }

    // 4) as fórmulas empacotadas: padrão (já carregado no passo 0) e valor
    //    direto na tabela colunar, emprestando os vetores lidos.
    let mut vazia = sheet.is_empty();
    for coluna in fx {
        let coluna = coluna.borrow();
        let erros: Vec<(&str, Option<&str>)> = coluna.erros.iter().map(|e| (e.code.as_str(), e.detail.as_deref())).collect();
        let linhas = coluna.slots.len() as u32;
        sheet.formulas.load_column_partes(coluna.col, &coluna.slots, &coluna.payload, &coluna.bitmap,
            &coluna.dict, &erros, linhas, &mut sheet.pool);
        estender_limites(sheet, coluna.col, &coluna.bitmap, linhas, &mut vazia);
        crate::progresso::etapa_somar(linhas as u64);
    }
}

/// A coluna carregada direto na tabela não passa pelos `set_*` que mantêm a
/// área usada da aba; sem isto a barra de rolagem e o quadro não a veriam.
fn estender_limites(
    sheet: &mut feathersheet_core::planilha::sheet::Sheet,
    col: u32,
    bitmap: &[u8],
    linhas: u32,
    vazia: &mut bool,
) {
    let presente = |i: u32| tem_bit(bitmap, i as usize);
    let Some(primeira) = (0..linhas).find(|&i| presente(i)) else { return };
    let ultima = (0..linhas).rev().find(|&i| presente(i)).unwrap_or(primeira);
    if std::mem::take(vazia) {
        sheet.min_row = primeira;
        sheet.max_row = ultima;
        sheet.min_col = col;
        sheet.max_col = col;
    } else {
        sheet.min_row = sheet.min_row.min(primeira);
        sheet.max_row = sheet.max_row.max(ultima);
        sheet.min_col = sheet.min_col.min(col);
        sheet.max_col = sheet.max_col.max(col);
    }
}

/// Se a linha `i` tem valor, segundo a máscara de presença do Arrow.
fn tem_bit(bitmap: &[u8], i: usize) -> bool {
    bitmap.get(i >> 3).is_some_and(|b| b & (1 << (i & 7)) != 0)
}

/// Traduz o estilo que veio do arquivo (JSON, como o TypeScript grava).
///
/// Campo que não se reconhece é ignorado em silêncio, de propósito: um
/// arquivo de uma versão mais nova não pode deixar de abrir por causa de um
/// atributo novo. É a mesma política do `unpack` do `io/fshz.ts`.
#[allow(dead_code)]
fn estilo_de_json(json: &serde_json::Value) -> Option<Style> {
    let obj = json.as_object()?;
    let bool_de = |k: &str| obj.get(k).and_then(|v| v.as_bool()).unwrap_or(false);
    let texto_de = |k: &str| obj.get(k).and_then(|v| v.as_str()).map(str::to_string);

    let estilo = Style {
        bold: bool_de("bold"),
        italic: bool_de("italic"),
        underline: bool_de("underline"),
        strike: bool_de("strike"),
        wrap: bool_de("wrap"),
        font_size: obj.get("fontSize").and_then(|v| v.as_f64()).map(|n| n as f32),
        font_family: texto_de("fontFamily"),
        decimals: obj.get("decimals").and_then(|v| v.as_u64()).map(|n| n as u8),
        borders: None,
        align: match texto_de("align").as_deref() {
            Some("left") => Align::Left,
            Some("center") => Align::Center,
            Some("right") => Align::Right,
            _ => Align::Auto,
        },
        valign: match texto_de("vAlign").as_deref() {
            Some("top") => VAlign::Top,
            Some("bottom") => VAlign::Bottom,
            _ => VAlign::Middle,
        },
        format: match texto_de("format").as_deref() {
            Some("number") => NumberFormat::Number,
            Some("integer") => NumberFormat::Integer,
            Some("currency") => NumberFormat::Currency,
            Some("scientific") => NumberFormat::Scientific,
            Some("accounting") => NumberFormat::Accounting,
            Some("duration") => NumberFormat::Duration,
            Some("percent") => NumberFormat::Percent,
            Some("date") => NumberFormat::Date,
            Some("datetime") => NumberFormat::Datetime,
            Some("time") => NumberFormat::Time,
            Some("text") => NumberFormat::Text,
            _ => NumberFormat::Auto,
        },
        color: texto_de("color"),
        bg: texto_de("bg"),
        rotation: obj.get("rotation").and_then(|v| v.as_i64()).map(|n| n as i16),
        link: texto_de("link"),
        code: texto_de("numFmt"),
    };
    if estilo.is_empty() {
        None
    } else {
        Some(estilo)
    }
}

pub fn estilo_para_json(estilo: &Style) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    if estilo.bold {
        map.insert("bold".into(), serde_json::Value::Bool(true));
    }
    if estilo.italic {
        map.insert("italic".into(), serde_json::Value::Bool(true));
    }
    if estilo.underline {
        map.insert("underline".into(), serde_json::Value::Bool(true));
    }
    if estilo.strike {
        map.insert("strike".into(), serde_json::Value::Bool(true));
    }
    if estilo.wrap {
        map.insert("wrap".into(), serde_json::Value::Bool(true));
    }
    if let Some(fs) = estilo.font_size {
        if fs.fract() == 0.0 {
            map.insert("fontSize".into(), serde_json::Value::from(fs as u64));
        } else if let Some(num) = serde_json::Number::from_f64(fs as f64) {
            map.insert("fontSize".into(), serde_json::Value::Number(num));
        }
    }
    if let Some(ref ff) = estilo.font_family {
        map.insert("fontFamily".into(), serde_json::Value::String(ff.clone()));
    }
    if let Some(dec) = estilo.decimals {
        map.insert("decimals".into(), serde_json::Value::from(dec));
    }
    match estilo.align {
        Align::Left => { map.insert("align".into(), serde_json::Value::String("left".into())); },
        Align::Center => { map.insert("align".into(), serde_json::Value::String("center".into())); },
        Align::Right => { map.insert("align".into(), serde_json::Value::String("right".into())); },
        Align::Auto => {},
    }
    match estilo.valign {
        VAlign::Top => { map.insert("vAlign".into(), serde_json::Value::String("top".into())); },
        VAlign::Bottom => { map.insert("vAlign".into(), serde_json::Value::String("bottom".into())); },
        VAlign::Middle => {},
    }
    match estilo.format {
        NumberFormat::Number => { map.insert("format".into(), serde_json::Value::String("number".into())); },
        NumberFormat::Integer => { map.insert("format".into(), serde_json::Value::String("integer".into())); },
        NumberFormat::Currency => { map.insert("format".into(), serde_json::Value::String("currency".into())); },
        NumberFormat::Scientific => { map.insert("format".into(), serde_json::Value::String("scientific".into())); },
        NumberFormat::Accounting => { map.insert("format".into(), serde_json::Value::String("accounting".into())); },
        NumberFormat::Duration => { map.insert("format".into(), serde_json::Value::String("duration".into())); },
        NumberFormat::Percent => { map.insert("format".into(), serde_json::Value::String("percent".into())); },
        NumberFormat::Date => { map.insert("format".into(), serde_json::Value::String("date".into())); },
        NumberFormat::Datetime => { map.insert("format".into(), serde_json::Value::String("datetime".into())); },
        NumberFormat::Time => { map.insert("format".into(), serde_json::Value::String("time".into())); },
        NumberFormat::Text => { map.insert("format".into(), serde_json::Value::String("text".into())); },
        NumberFormat::Auto => {},
    }
    if let Some(ref c) = estilo.color {
        map.insert("color".into(), serde_json::Value::String(c.clone()));
    }
    if let Some(ref bg) = estilo.bg {
        map.insert("bg".into(), serde_json::Value::String(bg.clone()));
    }
    if let Some(rot) = estilo.rotation {
        map.insert("rotation".into(), serde_json::Value::from(rot));
    }
    if let Some(ref codigo) = estilo.code {
        map.insert("numFmt".into(), serde_json::Value::String(codigo.clone()));
    }
    if let Some(ref link) = estilo.link {
        map.insert("link".into(), serde_json::Value::String(link.clone()));
    }
    serde_json::Value::Object(map)
}

#[cfg(test)]
mod testes {
    use super::*;
    use feathersheet_core::celulas::values::Value;

    #[test]
    fn estilo_ida_e_volta() {
        let json_original = serde_json::json!({
            "bold": true,
            "fontSize": 14,
            "align": "right",
            "format": "currency",
            "color": "#ff0000",
            // Sem ele o link de um `.xlsx` sumia na primeira gravação do
            // `.fshz` pela pasta do Rust (item 19, 23/09/2026).
            "link": "https://exemplo.com"
        });
        let estilo = estilo_de_json(&json_original).expect("deveria converter json para estilo");
        let json_reconstruido = estilo_para_json(&estilo);
        assert_eq!(json_original, json_reconstruido);
    }

    /// A planilha de exemplo do repositório: 22 mil fórmulas, várias abas,
    /// gravada pelo caminho de verdade. É o teste de ponta a ponta que importa
    /// — um `PastaLida` montado à mão provaria menos.
    fn exemplo() -> Option<PastaLida> {
        let caminho = concat!(env!("CARGO_MANIFEST_DIR"), "/../../exemplos/planilha-pesada.fshz");
        let bytes = std::fs::read(caminho).ok()?;
        crate::fshz_read::ler_fshz(&bytes).ok()
    }

    #[test]
    fn o_arquivo_de_exemplo_vira_pasta_sem_passar_pelo_javascript() {
        let Some(lida) = exemplo() else {
            // O arquivo não está aqui (checkout parcial): não falha o teste,
            // mas também não mente dizendo que passou.
            eprintln!("exemplos/planilha-pesada.fshz não encontrado — teste pulado");
            return;
        };

        let esperava_abas = lida.sheets.len();
        let pasta = para_workbook(&lida);
        assert_eq!(pasta.len(), esperava_abas, "perdeu aba no caminho");
        assert!(!pasta.nomes().is_empty());

        let aba = &pasta.aba(0).unwrap().sheet;
        let usado = aba.used_rect();
        assert!(
            usado.r1 > 0 && usado.c1 > 0,
            "a primeira aba voltou vazia: {usado:?}",
        );
    }

    #[test]
    fn os_valores_do_arquivo_chegam_iguais() {
        let Some(lida) = exemplo() else { return };

        // O que o arquivo diz que tem, contado direto do que foi lido.
        let primeira = &lida.sheets[0];
        let mut esperados: Vec<(u32, u32, Value)> = Vec::new();
        for coluna in &primeira.colunas_num {
            for (linha, valor) in coluna.values.iter().enumerate().take(200) {
                if tem_bit(&coluna.bitmap, linha) {
                    esperados.push((linha as u32, coluna.col, Value::Number(*valor)));
                }
            }
        }
        for coluna in &primeira.colunas_txt {
            for (linha, indice) in coluna.indices.iter().enumerate().take(200) {
                if tem_bit(&coluna.bitmap, linha) {
                    if let Some(t) = coluna.dict.get(*indice as usize) {
                        esperados.push((linha as u32, coluna.col, Value::Text(t.clone())));
                    }
                }
            }
        }
        // Célula avulsa (fórmula sem padrão, ou aba pequena demais para
        // virar coluna) também entra na conta, além das colunas Arrow acima.
        // Sem `.take()`: a ordem das avulsas não é a da planilha (é a de
        // `looseCells()`), e as poucas literais (o cabeçalho) podem vir
        // depois de milhares de fórmulas avulsas no exemplo atual.
        for celula in primeira.avulsas.iter() {
            if celula.v.starts_with('=') {
                continue;
            }
            let row = (celula.k / 16384) as u32;
            let col = (celula.k % 16384) as u32;
            esperados.push((row, col, Value::Empty));
            // O valor certo é o que o motor decide a partir do texto digitado
            // — o mesmo `input.rs` dos dois lados.
            let ultimo = esperados.len() - 1;
            esperados[ultimo].2 = feathersheet_core::celulas::input::parse_literal(celula.v.trim()).value;
        }
        assert!(
            !esperados.is_empty(),
            "o exemplo precisa ter célula para o teste valer",
        );

        let pasta = para_workbook(&lida);
        let aba = &pasta.aba(0).unwrap().sheet;
        for (linha, col, esperado) in esperados {
            assert_eq!(
                aba.get_value(linha, col),
                esperado,
                "a célula ({linha}, {col}) voltou diferente do arquivo",
            );
        }
    }

    #[test]
    fn a_formula_empacotada_volta_como_formula() {
        let Some(lida) = exemplo() else { return };
        let primeira = &lida.sheets[0];
        let Some(coluna) = primeira.colunas_formula.first() else {
            eprintln!("o exemplo não tem coluna de fórmula — teste pulado");
            return;
        };
        let Some(linha) = (0..coluna.slots.len()).find(|i| tem_bit(&coluna.bitmap, *i)) else {
            return;
        };

        let pasta = para_workbook(&lida);
        let aba = &pasta.aba(0).unwrap().sheet;
        let bruto = aba.get_raw(linha as u32, coluna.col);
        assert!(
            bruto.starts_with('='),
            "a fórmula da linha {linha} voltou como \"{bruto}\", não como fórmula",
        );
    }

    /// Uma aba de três linhas (cabeçalho + duas), com B e C números e D e E
    /// fórmulas de padrões diferentes — o formato de verdade do arquivo.
    fn aba_com_duas_colunas_de_formula() -> PastaLida {
        let num = |col: u32, vals: [f64; 3]| crate::fshz_read::ColunaNumLida {
            col,
            values: vals.to_vec(),
            bitmap: vec![0b110],
        };
        // O `slot` empacota o padrão nos 24 bits altos e o tipo do valor nos
        // 8 baixos (1 = número). O valor guardado em E é 999 de propósito:
        // a conta daria outro número, e é assim que se prova que o valor veio
        // do arquivo em vez de ser recalculado.
        let formula = |col: u32, padrao: i32, valores: [f64; 3]| crate::fshz_read::ColunaFormulaLida {
            col,
            slots: vec![0, (padrao << 8) | 1, (padrao << 8) | 1],
            payload: valores.to_vec(),
            bitmap: vec![0b110],
            dict: vec![],
            erros: vec![],
        };
        PastaLida {
            v: 2,
            a: 0,
            names: None,
            es: None,
            pr: None,
            mc: None,
            ex: None,
            fm: None,
            sheets: vec![AbaLida {
                estilos: Default::default(),
                n: "Planilha1".into(),
                tc: None,
                oc: false,
                wp: None,
                tp: None,
                pt: None,
                cm: None,
                lo: None,
                co: None,
                avulsas: vec![],
                estilos_diretos: vec![],
                estilos_faixa: vec![],
                estilos_coluna: vec![],
                estilos_linha: vec![],
                estilo_aba: None,
                colunas_num: vec![num(1, [0.0, 101.03, 300.0]), num(2, [0.0, 11.06, 5.0])],
                colunas_txt: vec![],
                colunas_formula: vec![formula(3, 0, [0.0, 112.09, 305.0]), formula(4, 1, [0.0, 999.0, 350.75])],
                padroes_formula: vec![
                    "=\u{1}r-2;r0\u{1}+\u{1}r-1;r0\u{1}".into(),
                    "=SE(\u{1}r-1;r0\u{1}>200;\u{1}r-1;r0\u{1}*1.15;\u{1}r-1;r0\u{1}*1.05)".into(),
                ],
                w: vec![],
                h: vec![],
                g: None,
                pr: None,
                r: None,
                tb: None,
                oq: None,
                dn: None,
                mc: None,
                im: None,
                pd: None,
                va: None,
                bt: None,
                tn: None,
                ob: None,
                ce: None,
            }],
        }
    }

    #[test]
    fn cada_coluna_de_formula_pega_o_seu_padrao() {
        // Bug de 22/09/2026, achado abrindo o arquivo do usuário: o `slot`
        // era usado inteiro como número do padrão, sem o `>> 8`. A coluna D
        // (padrão 0, tipo 1) pegava o padrão 1 — a fórmula da coluna E, uma
        // coluna para a esquerda —, e as colunas E e F ficavam vazias.
        let pasta = para_workbook(&aba_com_duas_colunas_de_formula());
        let aba = &pasta.aba(0).unwrap().sheet;
        assert_eq!(aba.get_raw(1, 3), "=B2+C2");
        assert_eq!(aba.get_raw(2, 3), "=B3+C3");
        assert_eq!(aba.get_raw(1, 4), "=SE(D2>200;D2*1.15;D2*1.05)");
    }

    #[test]
    fn a_formula_abre_com_o_valor_do_arquivo_sem_recalcular() {
        // O valor calculado vem no arquivo; jogá-lo fora obrigava o Rust a
        // recalcular os seis milhões de fórmulas do arquivo do usuário.
        let pasta = para_workbook(&aba_com_duas_colunas_de_formula());
        let aba = &pasta.aba(0).unwrap().sheet;
        assert_eq!(aba.get_value(1, 4), Value::Number(999.0), "recalculou em vez de usar o valor guardado");
        assert_eq!(aba.get_value(2, 4), Value::Number(350.75));
        assert_eq!(aba.get_value(1, 3), Value::Number(112.09));
        let usado = aba.used_rect();
        assert_eq!((usado.r1, usado.c1), (2, 4), "as fórmulas precisam contar na área usada");
    }

    #[test]
    fn mudar_a_entrada_suja_a_formula_carregada() {
        // Usar o valor do arquivo não pode prender a fórmula nele: quando a
        // entrada muda, ela recalcula.
        let mut pasta = para_workbook(&aba_com_duas_colunas_de_formula());
        let aba = &mut pasta.aba_mut(0).unwrap().sheet;
        aba.set_cell_raw(1, 1, "1".into(), None);
        assert_eq!(aba.get_value(1, 3), Value::Number(12.06));
        let e = match aba.get_value(1, 4) {
            Value::Number(n) => n,
            outro => panic!("E2 deu {outro:?}"),
        };
        assert!((e - 12.06 * 1.05).abs() < 1e-9, "E2 não acompanhou a mudança: {e}");
    }

    #[test]
    fn a_altura_zero_do_filtro_sobrevive() {
        // Já se perdeu uma vez neste projeto: a linha escondida pelo filtro é
        // altura zero, e se ela não voltar do arquivo o filtro reabre errado.
        let mut lida_falsa = PastaLida {
            v: 2,
            a: 0,
            names: None,
            es: None,
            pr: None,
            mc: None,
            ex: None,
            fm: None,
            sheets: vec![AbaLida {
                estilos: Default::default(),
                n: "Planilha1".into(),
                tc: None,
                oc: false,
                wp: None,
                tp: None,
                pt: None,
                cm: None,
                lo: None,
                co: None,
                avulsas: vec![],
                estilos_diretos: vec![],
                estilos_faixa: vec![],
                estilos_coluna: vec![],
                estilos_linha: vec![],
                estilo_aba: None,
                colunas_num: vec![],
                colunas_txt: vec![],
                colunas_formula: vec![],
                padroes_formula: vec![],
                w: vec![(2, 150.0)],
                h: vec![(7, 0.0)],
                g: None,
                pr: None,
                r: None,
                tb: None,
                oq: None,
                dn: None,
                mc: None,
                im: None,
                pd: None,
                va: None,
                bt: None,
                tn: None,
                ob: None,
                ce: None,
            }],
        };
        lida_falsa.sheets[0].avulsas.push(crate::fshz_read::CelulaAvulsaLida {
            k: 0,
            v: "topo".into(),
            estilo: None,
        });

        let pasta = para_workbook(&lida_falsa);
        let aba = &pasta.aba(0).unwrap().sheet;
        assert_eq!(aba.row_heights.get(&7), Some(&0), "a linha escondida voltou visível");
        assert_eq!(aba.col_widths.get(&2), Some(&150));
        assert_eq!(aba.get_value(0, 0), Value::Text("topo".into()));
    }
}
