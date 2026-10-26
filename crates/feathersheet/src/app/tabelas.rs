//! Criar Tabela (Ctrl+T) no FeatherSheet.
 //! Contrato JSON legado `TabelaDaAba` no campo `Sheet.tb`, sem copiar dados
 //! nem converter os milhões de células da grade em um objeto denso.
mod migracao;
mod painel;
pub(in crate::app) mod totais;
pub(in crate::app) mod totais_popup;
pub(in crate::app) mod filtro_popup;
pub(in crate::app) mod filtro_extra;
pub(crate) use painel::Acao;
pub(super) use migracao::{sob_celula, sob_qualquer, sob_id, renomear, redimensionar, estilo, Estilo, converter, filtrar, limpar_todos_filtros, ordenar, ordenar_por, reaplicar, crescer, crescer_colagem, distintos_lote, refluir, alternar_auto_filtro};

use feathersheet_core::{
    celulas::refs::{Rect, MAX_COLS, MAX_ROWS},
    operacoes::{ops, tabela},
    planilha::{sheet::Sheet, workbook::Workbook},
};
use pelicano::Color;
use serde_json::{json, Value};

fn listar(sheet: &Sheet) -> Result<Vec<Value>, String> {
    let Some(ref bruto) = sheet.tb else { return Ok(Vec::new()) };
    serde_json::from_str::<Vec<Value>>(bruto)
        .map_err(|_| tr!("Metadados de tabelas da aba estão inválidos; nada foi alterado.").into())
}

fn area_valida(area: Rect) -> Result<Rect, String> {
    let a = area.normalized();
    if a.r1 >= MAX_ROWS || a.c1 >= MAX_COLS {
        return Err(tr!("O intervalo está fora dos limites da grade.").into());
    }
    Ok(a)
}

fn area_de(valor: &Value) -> Option<Rect> {
    let a = &valor["area"];
    let r0 = u32::try_from(a["r0"].as_u64()?).ok()?;
    let r1 = u32::try_from(a["r1"].as_u64()?).ok()?;
    let c0 = u32::try_from(a["c0"].as_u64()?).ok()?;
    let c1 = u32::try_from(a["c1"].as_u64()?).ok()?;
    (r0 <= r1 && c0 <= c1 && r1 < MAX_ROWS && c1 < MAX_COLS)
        .then(|| Rect::new(r0, c0, r1, c1))
}

fn intersecta(a: &Rect, b: &Rect) -> bool {
    a.r0 <= b.r1 && b.r0 <= a.r1 && a.c0 <= b.c1 && b.c0 <= a.c1
}


/// Ao desfazer/refazer tb, concilia APENAS nomes pertencentes a tabelas
/// reais. Nomes definidos independentes não são modificados. A aba não
/// guarda cópias das células: comparamos apenas pequenas descrições JSON.
pub(super) fn sincronizar_nomes_historico(
    w: &mut Workbook, aba: usize, antes: Option<&str>, depois: Option<&str>,
) {
    let extrair = |raw: Option<&str>| -> Vec<(String, Rect)> {
        let Some(raw)=raw else { return Vec::new() };
        let Ok(tabelas)=serde_json::from_str::<Vec<Value>>(raw) else {return Vec::new()};
        tabelas.iter().filter_map(|t| {
            if t["soFiltro"] == true {return None;}
            Some((t["nome"].as_str()?.to_owned(),area_de(t)?))
        }).collect()
    };
    let anteriores=extrair(antes);
    let atuais=extrair(depois);
    let Some(nome_aba)=w.aba(aba).map(|a|a.sheet.name.clone()) else {return};
    // Remove apenas a referência que correspondia à tabela anterior.
    for (nome,area) in &anteriores {
        if atuais.iter().any(|(n,a)|n.eq_ignore_ascii_case(nome)&&a==area) {continue;}
        let esperado=Rect::na_aba(area.r0,area.c0,area.r1,area.c1,nome_aba.clone());
        if w.nomeados().iter().any(|n|n.nome.eq_ignore_ascii_case(nome)&&n.area==esperado){
            w.remover_nome(nome);
        }
    }
    // Reintroduz nomes ao desfazer uma conversão, renomeação ou criação.
    // Nunca sobrescreve um intervalo nomeado independente.
    for (nome,area) in &atuais {
        let esperado=Rect::na_aba(area.r0,area.c0,area.r1,area.c1,nome_aba.clone());
        let existe=w.nomeados().iter().any(|n|n.nome.eq_ignore_ascii_case(nome));
        if !existe {
            w.definir_nome(nome,esperado);
        }
    }
}

pub(super) fn nomes_usados(w: &Workbook) -> Result<Vec<String>, String> {
    let mut nomes = Vec::new();
    // O TypeScript inclui os intervalos nomeados ao sugerir TabelaN:
    // não oferecer nome que faria a criação falhar por colisão.
    nomes.extend(w.nomeados().into_iter().map(|n|n.nome));
    for i in 0..w.len() {
        if let Some(aba) = w.aba(i) {
            for t in listar(&aba.sheet)? {
                if let Some(nome) = t["nome"].as_str() {
                    nomes.push(nome.to_owned());
                }
            }
        }
    }
    Ok(nomes)
}

pub(super) fn nome_sugerido(w: &Workbook) -> Result<String, String> {
    Ok(tabela::nome_disponivel(&nomes_usados(w)?, tr!("Tabela")))
}

/// Usa a região atual do motor existente quando só há uma célula marcada.
/// Em célula totalmente vazia, sugere 6 linhas × 3 colunas se couber e não
/// houver nenhum dado no retângulo; confirmações jamais apagam valores.
pub(super) fn sugerir(w: &Workbook, aba: usize, selecao: Rect)
    -> Result<(Rect, bool, bool), String>
{
    let sheet = &w.aba(aba).ok_or(tr!("A aba selecionada não existe."))?.sheet;
    let a = area_valida(selecao)?;
    let unica = a.r0 == a.r1 && a.c0 == a.c1;
    let vazia = unica && sheet.get_raw(a.r0, a.c0).is_empty();
    if vazia {
        let (r1, c1) = (a.r0.checked_add(5), a.c0.checked_add(2));
        let (Some(r1), Some(c1)) = (r1, c1) else {
            return Err(tr!("Selecione uma área para criar a tabela nessa posição.").into());
        };
        if r1 >= MAX_ROWS || c1 >= MAX_COLS {
            return Err(tr!("Não há espaço para a tabela padrão 6×3.").into());
        }
        let area = Rect::new(a.r0, a.c0, r1, c1);
        if (a.r0..=r1).any(|r| (a.c0..=c1).any(|c| !sheet.get_raw(r, c).is_empty())) {
            return Err(tr!("Há dados vizinhos. Selecione o intervalo antes de criar a tabela.").into());
        }
        return Ok((area, true, true));
    }
    let region = if unica { ops::regiao_atual(sheet, a) } else { a };
    let region = area_valida(region)?;
    let cabecalho = ops::parece_cabecalho(region.clone(), |r, c| sheet.get_value(r, c));
    Ok((region, cabecalho, false))
}

pub(super) fn criar(
    w: &mut Workbook, indice: usize, area: Rect,
    nome: &str, cabecalho: bool, vazia: bool,
) -> Result<(), String> {
    let area = area_valida(area)?;
    if nome.is_empty() || nome.len() > 64 ||
        !nome.chars().next().is_some_and(|c| c.is_alphabetic() || c == '_') ||
        !nome.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return Err(tr!("Use um nome de tabela com letras, números e _, começando por letra.").into());
    }
    if nomes_usados(w)?.iter().any(|n| n.eq_ignore_ascii_case(nome)) {
        return Err(tr!("Já existe uma tabela com esse nome na pasta.").into());
    }
    let aba = w.aba(indice).ok_or(tr!("A aba selecionada não existe."))?;
    let mut tabelas = listar(&aba.sheet)?;
    if tabelas.iter().filter_map(area_de).any(|r| intersecta(&r, &area)) {
        return Err(tr!("A seleção cruza outra tabela. Escolha um intervalo livre.").into());
    }
    if vazia && (area.r0..=area.r1).any(|r|
        (area.c0..=area.c1).any(|c| !aba.sheet.get_raw(r,c).is_empty())) {
        return Err(tr!("A região não está mais vazia; nenhum dado foi sobrescrito.").into());
    }
    // A referência estruturada e o intervalo nomeado usam o MESMO nome.
    // Não sobrescrever um nome explícito que já existe na pasta.
    if w.nomeados().iter().any(|n| n.nome.eq_ignore_ascii_case(nome)) {
        return Err(tr!("Já existe um intervalo nomeado com esse nome.").into());
    }
    let id = format!("tab_{}", nome);
    tabelas.push(json!({
        "id": id, "nome": nome,
        "area": {"r0": area.r0, "c0": area.c0, "r1": area.r1, "c1": area.c1},
        "temCabecalho": cabecalho, "faixas": true,
        "estilo": {"tema": "azul", "faixasColuna": false, "borda": true},
        "filtros": []
    }));
    let dados = serde_json::to_string(&tabelas)
        .map_err(|e| trf!("Não foi possível armazenar tabela: {e}"))?;
    {
        let aba = w.aba_mut(indice).ok_or(tr!("A aba selecionada não existe."))?;
        // Registrar o objeto e os cabeçalhos criados numa só transação:
        // um Ctrl+Z remove a tabela e desfaz apenas os cabeçalhos gerados.
        aba.sheet.abrir_transacao();
        if aba.sheet.alterar_tabelas(Some(dados), &area) {
            aba.versao = aba.versao.wrapping_add(1).max(1);
        }
    }
    // Cabeçalhos só são escritos ao criar uma tabela NOVA sobre espaço vazio,
    // nunca sobre dados importados, fórmulas ou estilos existentes.
    if vazia && cabecalho {
        for (coluna, c) in (area.c0..=area.c1).enumerate() {
            w.escrever_celula(indice, area.r0, c, format!("Coluna{}", coluna + 1));
        }
    }
    w.aba_mut(indice).ok_or(tr!("A aba selecionada não existe."))?
        .sheet.fechar_transacao();
    // O TS registra cada tabela como intervalo nomeado: fórmulas como
    // =SOMA(Tabela1) continuam funcionando no FeatherSheet nativo.
    let nome_aba = w.aba(indice).ok_or(tr!("Aba selecionada não existe."))?.sheet.name.clone();
    w.definir_nome(nome, Rect::na_aba(area.r0, area.c0, area.r1, area.c1, nome_aba));
    Ok(())
}

/// Desenho virtualizado: somente metadados de cada tabela por frame.
#[derive(Clone, Debug)]
pub(crate) struct Visual {
    area: Rect,
    nome: String,
    cabecalho: bool,
    faixas: bool,
    faixas_coluna: bool,
    borda: bool,
    tema: String,
    mostrar_totais: bool,
    totais: Vec<(u32,String)>,
    filtros: Vec<u32>,
    so_filtro: bool,
}
/// Só os retângulos das tabelas e AutoFiltros da aba.
pub(crate) fn areas(sheet: &Sheet) -> Vec<Rect> {
    listar(sheet).unwrap_or_default().iter().filter_map(area_de).collect()
}

pub(crate) fn visuais(sheet: &Sheet) -> Vec<Visual> {
    listar(sheet).unwrap_or_default().iter().filter_map(|item| {
        Some(Visual {
            area: area_de(item)?,
            nome: item["nome"].as_str().unwrap_or("").to_owned(),
            cabecalho: item["temCabecalho"] == true,
            so_filtro: item["soFiltro"] == true,
            faixas: item["faixas"] == true,
            faixas_coluna: item["estilo"]["faixasColuna"] == true,
            borda: item["estilo"]["borda"] == true,
            tema: item["estilo"]["tema"].as_str().unwrap_or("azul").to_owned(),
            mostrar_totais: item["mostrarTotais"] == true,
            totais: item["totais"].as_array().map(|lista| lista.iter().filter_map(|t| {
                Some((u32::try_from(t["col"].as_u64()?).ok()?,
                    t["fn"].as_str()?.to_owned()))
            }).collect()).unwrap_or_default(),
            filtros: item["filtros"].as_array().map(|lista| lista.iter()
                .filter_map(|t| t["col"].as_u64().and_then(|c|u32::try_from(c).ok()))
                .collect()).unwrap_or_default(),
        })
    }).collect()
}

/// Nome da tabela cuja área é exatamente a seleção (a caixa de nome mostra
/// "Tabela1" nesse caso, como no Excel). Um AutoFiltro puro não tem nome.
pub(crate) fn nome_da_area_exata<'a>(visuais: &'a [Visual], area: &Rect) -> Option<&'a str> {
    visuais.iter()
        .find(|v| !v.so_filtro && !v.nome.is_empty() && v.area == *area)
        .map(|v| v.nome.as_str())
}

/// Cores das seis paletas TS (versões clara e escura, sem persistir hex).
/// Também é a fonte das miniaturas da galeria de estilos (`estilos_popup`).
pub(super) fn paleta(tema: &str, escuro: bool) -> (Color,Color,Color) {
    let (cab, faixa, borda) = match (tema,escuro) {
        ("verde",false) => ([19,115,51],[230,244,234],[19,115,51]),
        ("verde",true) => ([20,83,45],[23,43,31],[47,125,79]),
        ("laranja",false) => ([197,87,10],[253,236,217],[197,87,10]),
        ("laranja",true) => ([124,58,6],[51,36,26],[180,108,34]),
        ("vermelho",false) => ([179,38,30],[252,232,230],[179,38,30]),
        ("vermelho",true) => ([110,27,22],[51,32,31],[168,66,58]),
        ("roxo",false) => ([107,63,160],[241,233,251],[107,63,160]),
        ("roxo",true) => ([66,39,107],[39,32,51],[127,91,176]),
        ("cinza",false) => ([95,99,104],[241,243,244],[95,99,104]),
        ("cinza",true) => ([60,64,67],[38,40,43],[107,112,117]),
        (_,true) => ([27,79,156],[28,43,63],[47,107,196]),
        (_,false) => ([26,115,232],[232,240,254],[26,115,232]),
    };
    (Color::rgb8(cab[0],cab[1],cab[2]),
     Color::rgb8(faixa[0],faixa[1],faixa[2]),
     Color::rgb8(borda[0],borda[1],borda[2]))
}
/// A paleta e as faixas são apenas desenho e não escrevem estilo nas células.
pub(crate) fn fundo(visuais: &[Visual], r: u32, c: u32, escuro: bool) -> Option<Color> {
    for v in visuais {
        if v.so_filtro {continue;}
        let a=&v.area;
        if c<a.c0 || c>a.c1 || r<a.r0 || r>a.r1+u32::from(v.mostrar_totais) { continue; }
        let (cab,faixa,_) = paleta(&v.tema,escuro);
        if v.cabecalho && r==a.r0 { return Some(cab); }
        if v.mostrar_totais && r==a.r1+1 {return Some(faixa);}
        let por_linha = v.faixas && (r-a.r0-u32::from(v.cabecalho))%2==1;
        let por_coluna = v.faixas_coluna && (c-a.c0)%2==1;
        if por_linha || por_coluna { return Some(faixa); }
    }
    None
}
pub(crate) fn cabecalho(visuais: &[Visual], r: u32, c: u32) -> bool {
    visuais.iter().any(|v| v.cabecalho && v.area.r0==r &&
        c>=v.area.c0 && c<=v.area.c1)
}
pub(crate) fn icone_filtro(visuais: &[Visual], r: u32, c: u32) -> Option<bool> {
    visuais.iter().find(|v|v.cabecalho && r==v.area.r0 &&
        c>=v.area.c0 && c<=v.area.c1)
        .map(|v|v.filtros.contains(&(c-v.area.c0)))
}
/// Botão de seta da célula de total (só na célula ativa, como no Excel).
pub(crate) fn e_celula_de_total(visuais: &[Visual], r: u32, c: u32) -> bool {
    visuais.iter().any(|v|!v.so_filtro && v.mostrar_totais &&
        r==v.area.r1+1 && c>=v.area.c0 && c<=v.area.c1)
}
pub(crate) fn tem_filtro_cabecalho(sheet: &Sheet, r: u32, c: u32) -> bool {
    icone_filtro(&visuais(sheet),r,c).is_some()
}
pub(crate) fn borda(visuais: &[Visual], r: u32, c: u32, escuro: bool) -> Option<(bool,bool,bool,bool,Color)> {
    let v=visuais.iter().find(|v|!v.so_filtro && v.borda && c>=v.area.c0 &&
        c<=v.area.c1 && r>=v.area.r0 &&
        r<=v.area.r1+u32::from(v.mostrar_totais))?;
    let (_,_,cor)=paleta(&v.tema,escuro);
    let fim=v.area.r1+u32::from(v.mostrar_totais);
    Some((r==v.area.r0,r==fim,c==v.area.c0,c==v.area.c1,cor))
}
/// Renderiza totais sem fórmulas gravadas: respeita as linhas escondidas.
pub(crate) fn total_texto(w: &Workbook, aba: usize, visuais: &[Visual], r: u32, c: u32) -> Option<String> {
    let v=visuais.iter().find(|v|v.mostrar_totais && r==v.area.r1+1 &&
        c>=v.area.c0 && c<=v.area.c1)?;
    let funcao=v.totais.iter().find(|(col,_)|*col==c-v.area.c0)
        .map(|(_,f)|f.as_str()).unwrap_or("nenhum");
    // Desde a linha de totais com células reais, só arquivos antigos (total
    // apenas desenhado, célula vazia) caem aqui; célula preenchida vale.
    if funcao=="nenhum" || !w.aba(aba)?.sheet.get_raw(r,c).is_empty() {return None;}
    let primeira=v.area.r0+u32::from(v.cabecalho);
    if primeira>v.area.r1 {return Some(String::new());}
    let sheet=&w.aba(aba)?.sheet;
    let valor=feathersheet_core::operacoes::tabela::total_da_coluna(
        sheet,Rect::new(primeira,c,v.area.r1,c),funcao,
        |rr|sheet.row_heights.get(&rr).copied()!=Some(0),
        |rr,cc|w.valor(aba,rr,cc));
    // A versão TS usa o estilo da última linha de dados para formatar
    // o total (data/moeda/porcentagem), não a conversão cru `f64::to_string`.
    let estilo=sheet.estilo_resolvido(v.area.r1,c);
    Some(valor.map(|n|
        feathersheet_core::celulas::style::format_value(&feathersheet_core::Value::Number(n), estilo.as_ref())
    ).unwrap_or_default())
}

#[cfg(test)]
mod testes {
    use super::*;
    #[test]
    fn nome_sugerido_respeita_nomes_de_intervalos() {
        let mut w=Workbook::new();
        w.definir_nome("Tabela1",Rect::new(0,0,2,1));
        assert_eq!(nome_sugerido(&w).unwrap(),"Tabela2");
        assert!(criar(&mut w,0,Rect::new(3,0,5,2),"Tabela1",true,false).is_err());
        criar(&mut w,0,Rect::new(3,0,5,2),"Tabela2",true,false).unwrap();
        assert_eq!(nome_sugerido(&w).unwrap(),"Tabela3");
    }

    #[test]
    fn cria_intervalo_real_sem_tocar_numeros_formula_e_fshz() {
        let mut w = Workbook::new();
        w.escrever_celula(0, 0, 0, "Produto".into());
        w.escrever_celula(0, 0, 1, "Quantidade".into());
        w.escrever_celula(0, 1, 0, "Caneta".into());
        w.escrever_celula(0, 1, 1, "3".into());
        w.escrever_celula(0, 2, 0, "Lapis".into());
        w.escrever_celula(0, 2, 1, "=B2*2".into());
        let (area, cab, vazio) = sugerir(&w, 0, Rect::new(0, 0, 0, 0)).unwrap();
        assert_eq!(area, Rect::new(0, 0, 2, 1));
        assert!(cab);
        assert!(!vazio);
        let antes = w.aba(0).unwrap().sheet.get_raw(2, 1);
        criar(&mut w, 0, area, "Tabela1", cab, vazio).unwrap();
        assert_eq!(w.aba(0).unwrap().sheet.get_raw(2, 1), antes);
        assert_eq!(w.aba(0).unwrap().sheet.get_raw(1, 1), "3");
        let t: Vec<Value> = listar(&w.aba(0).unwrap().sheet).unwrap();
        assert_eq!(t[0]["nome"], "Tabela1");
        assert_eq!(t[0]["temCabecalho"], true);
        let bytes = feathersheet_io::da_workbook::pasta_gravar_bytes(&w, 0).unwrap();
        let lido = feathersheet_io::fshz_read::ler_fshz(&bytes).unwrap();
        let reaberto = feathersheet_io::para_workbook::para_workbook(&lido);
        assert_eq!(listar(&reaberto.aba(0).unwrap().sheet).unwrap(), t);
        assert_eq!(reaberto.aba(0).unwrap().sheet.get_raw(2, 1), antes);
    }

    #[test]
    fn vazia_gera_cabecalho_e_colunas_sem_apagar_valores() {
        let mut w = Workbook::new();
        let (area, cab, vazia) = sugerir(&w,0,Rect::new(0,0,0,0)).unwrap();
        assert_eq!(area, Rect::new(0,0,5,2));
        criar(&mut w,0,area,"Tabela1",cab,vazia).unwrap();
        assert_eq!(w.aba(0).unwrap().sheet.get_raw(0,0),"Coluna1");
        assert_eq!(w.aba(0).unwrap().sheet.get_raw(0,2),"Coluna3");
        let vis = visuais(&w.aba(0).unwrap().sheet);
        assert!(cabecalho(&vis,0,0));
        assert!(fundo(&vis,0,1,false).is_some());
        assert!(fundo(&vis,2,1,false).is_some());
        assert!(fundo(&vis,1,1,false).is_none());
    }

    #[test]
    fn nome_da_tabela_aparece_so_na_area_exata_sem_alterar_celulas() {
        let mut w = Workbook::new();
        criar(&mut w, 0, Rect::new(2, 1, 4, 3), "Tabela1", true, false).unwrap();
        let visuais = visuais(&w.aba(0).unwrap().sheet);
        assert_eq!(nome_da_area_exata(&visuais, &Rect::new(2, 1, 4, 3)), Some("Tabela1"));
        assert_eq!(nome_da_area_exata(&visuais, &Rect::new(3, 2, 3, 2)), None);
        assert_eq!(nome_da_area_exata(&visuais, &Rect::new(0, 0, 0, 0)), None);
        assert!(w.aba(0).unwrap().sheet.get_raw(2, 1).is_empty());
    }

    #[test]
    fn sobreposicao_nome_duplicado_e_json_invalido_rejeitados() {
        let mut w = Workbook::new();
        criar(&mut w,0,Rect::new(0,0,3,2),"Tabela1",true,false).unwrap();
        let antes = w.aba(0).unwrap().sheet.tb.clone();
        assert!(criar(&mut w,0,Rect::new(2,2,5,4),"Tabela2",true,false).is_err());
        assert!(criar(&mut w,0,Rect::new(5,5,7,7),"tabela1",true,false).is_err());
        assert_eq!(w.aba(0).unwrap().sheet.tb,antes);
        assert_eq!(nome_sugerido(&w).unwrap(),"Tabela2");
        w.aba_mut(0).unwrap().sheet.tb=Some("{invalido".into());
        assert!(nome_sugerido(&w).is_err());
        assert!(criar(&mut w,0,Rect::new(6,6,7,8),"Tabela2",true,false).is_err());
    }
}
