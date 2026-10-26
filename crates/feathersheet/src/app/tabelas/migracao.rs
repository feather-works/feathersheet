//! Paridade das operações de TabelaDaAba do TypeScript em Rust.
//! Operações sobre JSON legado isoladas da engine colunar; filtros, ordenação
//! e totais delegam aos algoritmos do feathersheet-core.
use super::{area_de, area_valida, listar, nomes_usados};
use feathersheet_core::{
    celulas::{refs::Rect, values::Value as Valor},
    operacoes::{filtro, ops::{self, SortKey}, tabela::{self, AreaDaTabela}},
    operacoes::ordenacao_ext::{self, ChaveOrdem, CriterioOrdem},
    planilha::workbook::Workbook,
};
use feathersheet_io::filtro_json;
use serde_json::{json, Value as Json};
use std::collections::HashSet;

/// Recupera uma tabela verdadeira que contém a célula; o AutoFiltro é separado.
pub(in crate::app) fn sob_qualquer(w: &Workbook, aba: usize, r: u32, c: u32) -> Option<Json> {
    listar(&w.aba(aba)?.sheet).ok()?.into_iter()
        .find(|t|area_de(t).is_some_and(|a|a.contains(r,c)))
}
pub(in crate::app) fn sob_celula(w: &Workbook, aba: usize, r: u32, c: u32) -> Option<Json> {
    sob_qualquer(w,aba,r,c).filter(|t|t["soFiltro"]!=true)
}

fn indice(tabelas: &[Json], id: &str) -> Option<usize> {
    tabelas.iter().position(|t| t["id"].as_str() == Some(id))
}

fn salvar(w: &mut Workbook, aba: usize, tabelas: &[Json]) -> Result<(), String> {
    let raw = serde_json::to_string(tabelas).map_err(|e| e.to_string())?;
    let viva = w.aba_mut(aba).ok_or(tr!("Aba ausente."))?;
    viva.sheet.tb = Some(raw);
    viva.versao = viva.versao.wrapping_add(1).max(1);
    Ok(())
}

/// Registra uma mudança de objeto de tabela sem percorrer suas células.
/// Só utilizar quando não houver alturas de filtro a restaurar.
fn salvar_com_historico(w: &mut Workbook, aba: usize, tabelas: &[Json], area: &Rect) -> Result<(), String> {
    let bruto = serde_json::to_string(tabelas).map_err(|e|e.to_string())?;
    let viva = w.aba_mut(aba).ok_or(tr!("Aba ausente."))?;
    if viva.sheet.alterar_tabelas(Some(bruto),area) {
        viva.versao = viva.versao.wrapping_add(1).max(1);
    }
    Ok(())
}

fn mudar(w: &mut Workbook, aba: usize, id: &str,
    f: impl FnOnce(&mut Json) -> Result<(), String>) -> Result<Json, String>
{
    let mut tabelas = listar(&w.aba(aba).ok_or(tr!("Aba ausente."))?.sheet)?;
    let i = indice(&tabelas, id).ok_or(tr!("Tabela não encontrada."))?;
    f(&mut tabelas[i])?;
    let retorno = tabelas[i].clone();
    salvar(w, aba, &tabelas)?;
    Ok(retorno)
}

/// Somente mudanças de estilo/totais: não movimentam linhas nem mexem nos
/// nomes definidos, portanto cabem no histórico JSON compacto do Sheet.
/// Outras operações (filtros, áreas, renomeação) necessitam registrar também
/// alturas ou nomes e continuam fora desta trilha até serem transacionais.
fn mudar_estilo_com_historico(
    w: &mut Workbook, aba: usize, id: &str,
    f: impl FnOnce(&mut Json) -> Result<(), String>,
) -> Result<(), String> {
    let mut tabelas = listar(&w.aba(aba).ok_or(tr!("Aba ausente."))?.sheet)?;
    let i = indice(&tabelas,id).ok_or(tr!("Tabela não encontrada."))?;
    let area = area_de(&tabelas[i]).ok_or(tr!("Área de tabela inválida."))?;
    f(&mut tabelas[i])?;
    let novo = serde_json::to_string(&tabelas).map_err(|e|e.to_string())?;
    let viva = w.aba_mut(aba).ok_or(tr!("Aba ausente."))?;
    if viva.sheet.alterar_tabelas(Some(novo), &area) {
        viva.versao = viva.versao.wrapping_add(1).max(1);
    }
    Ok(())
}

fn grafia(nome: &str) -> bool {
    nome.len() <= 64
        && nome.chars().next().is_some_and(|c| c.is_alphabetic() || c == '_')
        && nome.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '.')
}

pub(in crate::app) fn renomear(w: &mut Workbook, aba: usize, id: &str, novo: &str) -> Result<(), String> {
    let nome = novo.trim();
    if !grafia(nome) { return Err(tr!("Nome inválido (letra, número, _ ou ponto).").into()); }
    let atual = sob_id(w, aba, id)?;
    let antigo = atual["nome"].as_str().ok_or(tr!("Tabela sem nome."))?.to_string();
    if antigo.eq_ignore_ascii_case(nome) { return Ok(()); }
    if nomes_usados(w)?.iter().any(|n| n.eq_ignore_ascii_case(nome))
        || w.nomeados().iter().any(|n| n.nome.eq_ignore_ascii_case(nome)) {
        return Err(tr!("O nome já está em uso na pasta.").into());
    }
    let area = area_de(&atual).ok_or(tr!("Intervalo inválido."))?;
    mudar_estilo_com_historico(w, aba, id, |t| { t["nome"] = json!(nome); Ok(()) })?;
    w.remover_nome(&antigo);
    let nome_aba = w.aba(aba).ok_or(tr!("Aba ausente."))?.sheet.name.clone();
    w.definir_nome(nome, Rect::na_aba(area.r0, area.c0, area.r1, area.c1, nome_aba));
    Ok(())
}

pub(in crate::app) fn sob_id(w: &Workbook, aba: usize, id: &str) -> Result<Json, String> {
    listar(&w.aba(aba).ok_or(tr!("Aba ausente."))?.sheet)?
        .into_iter().find(|v| v["id"].as_str() == Some(id))
        .ok_or_else(|| tr!("Tabela não encontrada.").into())
}

pub(in crate::app) enum Estilo {
    Faixas(bool), FaixasColuna(bool), Borda(bool), Tema(String),
    Totais(bool), TotalColuna { col: u32, funcao: String },
}
pub(in crate::app) fn estilo(w: &mut Workbook, aba: usize, id: &str, acao: Estilo) -> Result<(), String> {
    mudar_estilo_com_historico(w, aba, id, |t| {
        match acao {
            Estilo::Faixas(v) => t["faixas"] = json!(v),
            Estilo::FaixasColuna(v) => t["estilo"]["faixasColuna"] = json!(v),
            Estilo::Borda(v) => t["estilo"]["borda"] = json!(v),
            Estilo::Tema(ref s) => {
                if !["padrao","azul","verde","laranja","vermelho","roxo","cinza"].contains(&s.as_str()) {
                    return Err(tr!("Paleta desconhecida.").into());
                }
                t["estilo"]["tema"] = json!(s);
            },
            Estilo::Totais(v) => t["mostrarTotais"] = json!(v),
            Estilo::TotalColuna { col, ref funcao } => {
                let area = area_de(t).ok_or(tr!("Área de tabela inválida."))?;
                if col > area.c1 - area.c0 { return Err(tr!("Coluna fora da tabela.").into()); }
                if !["nenhum","soma","media","contagem","contagem-numeros",
                      "maximo","minimo","desvio","variancia"].contains(&funcao.as_str()) {
                    return Err(tr!("Função de total desconhecida.").into());
                }
                let mut lista = t["totais"].as_array().cloned().unwrap_or_default();
                lista.retain(|v| v["col"].as_u64() != Some(u64::from(col)));
                if funcao != "nenhum" { lista.push(json!({"col":col,"fn":funcao})); }
                t["totais"] = json!(lista);
            },
        }
        Ok(())
    })?;
    Ok(())
}


/** Atualiza a area sem modificar os valores da engine esparsa/colunar. */
pub(in crate::app) fn redimensionar(
    w: &mut Workbook, aba: usize, id: &str, novo: Rect,
) -> Result<(), String> {
    let novo = super::area_valida(novo)?;
    let mut tabelas = listar(&w.aba(aba).ok_or(tr!("Aba ausente."))?.sheet)?;
    let i = indice(&tabelas, id).ok_or(tr!("Tabela não encontrada."))?;
    let antigo = tabelas[i].clone();
    if antigo["soFiltro"] == true {
        return Err(tr!("AutoFiltro não pode ser redimensionado como tabela.").into());
    }
    let anterior = area_de(&antigo).ok_or(tr!("Intervalo inválido."))?;
    if novo.r0 != anterior.r0 || novo.c0 != anterior.c0 {
        return Err(tr!("Mantenha a primeira célula da tabela ao redimensionar.").into());
    }
    if antigo["temCabecalho"] == true && novo.r1 == novo.r0 {
        return Err(tr!("A tabela precisa de pelo menos uma linha de dados.").into());
    }
    if antigo["mostrarTotais"] == true
        && novo.r1 >= feathersheet_core::celulas::refs::MAX_ROWS - 1
    {
        return Err(tr!("Reserve uma linha para os totais abaixo da tabela.").into());
    }
    if antigo["mostrarTotais"] == true {
        return Err(tr!("Desligue a linha de totais antes de redimensionar a tabela.").into());
    }
    let mut destino = novo.clone();
    if antigo["mostrarTotais"] == true { destino.r1 += 1; }
    // Respeita tambem a linha virtual de totais de outra tabela.
    if tabelas.iter().enumerate().any(|(j, t)| {
        if i == j {return false;}
        area_de(t).is_some_and(|mut outra| {
            if t["mostrarTotais"] == true { outra.r1 = outra.r1.saturating_add(1); }
            super::intersecta(&destino, &outra)
        })
    }) {
        return Err(tr!("O intervalo cruza outra tabela ou uma linha de totais.").into());
    }
    if novo.r1 == anterior.r1 && novo.c1 == anterior.c1 {return Ok(());}
    let mut atual = antigo.clone();
    atual["area"] = json!({"r0":novo.r0,"c0":novo.c0,"r1":novo.r1,"c1":novo.c1});
    let ultima_coluna = u64::from(novo.c1 - novo.c0);
    for chave in ["filtros","totais","ordenar"] {
        if let Some(itens) = atual[chave].as_array_mut() {
            itens.retain(|item|item["col"].as_u64().is_some_and(|c|c<=ultima_coluna));
        }
    }
    let primeiro_filtro = atual["filtros"].as_array().and_then(|v|v.first())
        .and_then(|f|Some((u32::try_from(f["col"].as_u64()?).ok()?,f["criterio"].clone())));
    let nome = atual["nome"].as_str().unwrap_or("").to_owned();
    // Restaurar antes de mudar os limites evita deixar linhas ocultas fora
    // do novo intervalo (inclusive quando a coluna filtrada foi removida).
    limpar_visibilidade(w,aba,&antigo)?;
    atual["_featherOcultas"] = json!([]);
    atual["_featherAlturas"] = json!([]);
    tabelas[i] = atual;
    if primeiro_filtro.is_none() {
        salvar_com_historico(w,aba,&tabelas,&anterior)?;
    } else {
        // O filtro também muda alturas das linhas: só o JSON não seria
        // suficiente para desfazer corretamente esse redimensionamento.
        salvar(w,aba,&tabelas)?;
    }
    if !nome.is_empty() {
        let nome_aba = w.aba(aba).ok_or(tr!("Aba ausente."))?.sheet.name.clone();
        w.definir_nome(&nome,Rect::na_aba(novo.r0,novo.c0,novo.r1,novo.c1,nome_aba));
    }
    // filtrar avalia todos os criterios da tabela, nao somente o primeiro.
    if let Some((col,crit))=primeiro_filtro {
        filtrar(w,aba,id,col,Some(crit))?;
    }
    Ok(())
}

/// Remove só o objeto da tabela: as células não são tocadas.
pub(in crate::app) fn converter(w: &mut Workbook, aba: usize, id: &str) -> Result<(), String> {
    let antigo = sob_id(w, aba, id)?;
    // Não deixa alturas aplicadas pelo filtro após remover o objeto.
    limpar_visibilidade(w, aba, &antigo)?;
    let mut lista = listar(&w.aba(aba).ok_or(tr!("Aba ausente."))?.sheet)?;
    let i = indice(&lista, id).ok_or(tr!("Tabela não encontrada."))?;
    lista.remove(i);
    if antigo["filtros"].as_array().is_some_and(|fs|!fs.is_empty()) {
        // A conversão de tabela filtrada altera visibilidade de linhas.
        // Ainda não gerar um Ctrl+Z parcial que reabriria apenas o JSON.
        salvar(w,aba,&lista)?;
    } else {
        let area=area_de(&antigo).ok_or(tr!("Área de tabela inválida."))?;
        salvar_com_historico(w,aba,&lista,&area)?;
    }
    if let Some(nome) = antigo["nome"].as_str() { w.remover_nome(nome); }
    Ok(())
}

fn faixas(t: &Json, w: &mut Workbook, aba: usize) -> Result<Vec<(u32,u32)>, String> {
    let area = area_de(t).ok_or(tr!("Área de tabela inválida."))?;
    let primeira = area.r0 + u32::from(t["temCabecalho"] == true);
    let ultima = area.r1.min(w.aba(aba).ok_or(tr!("Aba ausente."))?.sheet.max_row);
    if primeira > ultima { return Ok(Vec::new()); }
    let mut filtros = Vec::new();
    if let Some(fs) = t["filtros"].as_array() {
        for f in fs {
            let rel = f["col"].as_u64().ok_or(tr!("Filtro sem coluna."))?;
            if rel > u64::from(area.c1 - area.c0) { continue; }
            filtros.push((area.c0 + rel as u32, filtro_json::criterio_de_json(&f["criterio"])));
        }
    }
    if filtros.is_empty() { return Ok(Vec::new()); }
    // Mesmo contexto de `filtro::linhas_escondidas_da_aba` (valor, cores e
    // "hoje"), mas o valor vem da pasta: fórmula que lê outra aba também vale.
    let sheet = &w.aba(aba).ok_or(tr!("Aba ausente."))?.sheet;
    let valor = |r, c| w.valor(aba, r, c);
    let cores = |r, c| filtro::cores_da_celula(sheet, r, c);
    Ok(filtro::linhas_escondidas_com(primeira, ultima, &filtros, &filtro::Contexto {
        valor: &valor, cores: Some(&cores), hoje: super::filtro_extra::serial_de_hoje(),
    }))
}

fn intervalos_lidos(t: &Json) -> Vec<(u32,u32)> {
    t["_featherOcultas"].as_array().map(|a| a.iter().filter_map(|v| {
        let linha = v.as_array()?;
        let (a,b) = (u32::try_from(linha.first()?.as_u64()?).ok()?,
                     u32::try_from(linha.get(1)?.as_u64()?).ok()?);
        (a <= b).then_some((a,b))
    }).collect()).unwrap_or_default()
}

fn limpar_visibilidade(w: &mut Workbook, aba: usize, t: &Json) -> Result<(), String> {
    let anteriores = intervalos_lidos(t);
    let backups: Vec<(u32,u32)> = t["_featherAlturas"].as_array().map(|a| a.iter().filter_map(|v| {
        let i = v.as_array()?;
        Some((u32::try_from(i.first()?.as_u64()?).ok()?,
              u32::try_from(i.get(1)?.as_u64()?).ok()?))
    }).collect()).unwrap_or_default();
    let viva = w.aba_mut(aba).ok_or(tr!("Aba ausente."))?;
    for (a,b) in anteriores {
        for r in a..=b { viva.sheet.row_heights.remove(&r); }
    }
    for (r,h) in backups { viva.sheet.row_heights.insert(r,h); }
    Ok(())
}

/// Remove todos os critérios de uma tabela sem tocar no conteúdo das células.
pub(in crate::app) fn limpar_todos_filtros(
    w: &mut Workbook, aba: usize, id: &str
) -> Result<usize,String> {
    let antigo = sob_id(w,aba,id)?;
    let quantidade = antigo["filtros"].as_array().map_or(0,Vec::len);
    if quantidade == 0 {return Ok(0);}
    let anteriores: usize = intervalos_lidos(&antigo).iter().map(|(a,b)| (b-a+1) as usize).sum();
    if anteriores > LIMITE_HISTORICO_FILTRO {
        limpar_visibilidade(w,aba,&antigo)?;
        mudar(w,aba,id,|t| {
            t["filtros"] = json!([]);
            t["_featherOcultas"] = json!([]);
            t["_featherAlturas"] = json!([]);
            Ok(())
        })?;
        return Ok(quantidade);
    }
    let area = area_de(&antigo).ok_or(tr!("Área de tabela inválida."))?;
    w.aba_mut(aba).ok_or(tr!("Aba ausente."))?.sheet.abrir_transacao();
    let feito = restaurar_visibilidade_registrada(w,aba,&antigo).and_then(|_| {
        let mut tabelas = listar(&w.aba(aba).ok_or(tr!("Aba ausente."))?.sheet)?;
        let i = indice(&tabelas,id).ok_or(tr!("Tabela não encontrada."))?;
        tabelas[i]["filtros"] = json!([]);
        tabelas[i]["_featherOcultas"] = json!([]);
        tabelas[i]["_featherAlturas"] = json!([]);
        salvar_com_historico(w,aba,&tabelas,&area)
    });
    let viva = w.aba_mut(aba).ok_or(tr!("Aba ausente."))?;
    viva.sheet.fechar_transacao();
    viva.versao = viva.versao.wrapping_add(1).max(1);
    feito?;
    Ok(quantidade)
}

/// Atualiza critérios e alturas sem cálculo por célula através da ponte TS.
pub(in crate::app) fn filtrar(w: &mut Workbook, aba: usize, id: &str, col_rel: u32, novo: Option<Json>) -> Result<usize, String> {
    let antigo = sob_id(w, aba, id)?;
    let area = area_de(&antigo).ok_or(tr!("Área inválida."))?;
    if col_rel > area.c1 - area.c0 { return Err(tr!("Coluna fora da tabela.").into()); }
    let mut atualizado = antigo.clone();
    let mut filtros = atualizado["filtros"].as_array().cloned().unwrap_or_default();
    filtros.retain(|f| f["col"].as_u64() != Some(u64::from(col_rel)));
    if let Some(regra) = novo { filtros.push(json!({"col":col_rel,"criterio":regra})); }
    atualizado["filtros"] = json!(filtros);
    // Primeiro calcula os predicados; se houver erro, nada é gravado/alterado.
    let ocultas = faixas(&atualizado, w, aba)?;
    let contagem: usize = ocultas.iter().map(|(a,b)| (b-a+1) as usize).sum();
    // Alturas e JSON num só Ctrl+Z, como o "Ocultar linhas". Acima do teto
    // (o mesmo do ocultar manual) o passo a passo pesaria na memória: aí o
    // filtro segue fora do histórico, como era antes.
    let anteriores: usize = intervalos_lidos(&antigo).iter().map(|(a,b)| (b-a+1) as usize).sum();
    let registrar = contagem + anteriores <= LIMITE_HISTORICO_FILTRO;
    if registrar {
        w.aba_mut(aba).ok_or(tr!("Aba ausente."))?.sheet.abrir_transacao();
        restaurar_visibilidade_registrada(w, aba, &antigo)?;
    } else {
        limpar_visibilidade(w, aba, &antigo)?;
    }
    let mut alturas = Vec::new();
    {
        let viva = w.aba_mut(aba).ok_or(tr!("Aba ausente."))?;
        for (a,b) in &ocultas {
            for r in *a..=*b {
                let antes = viva.sheet.row_heights.get(&r).copied();
                if registrar { viva.sheet.registrar_dimensao(false, r, Some(0)); }
                else { viva.sheet.row_heights.insert(r, 0); }
                if let Some(h) = antes {
                    // Se ja estava manualmente oculta (h=0), guardar
                    // essa altura para nao reexibi-la na limpeza.
                    alturas.push((r,h));
                }
            }
        }
    }
    // limpar_visibilidade já restaurou TODAS as alturas anteriores,
    // inclusive as linhas ocultas manualmente (altura 0). Ao inserir as
    // novas ocultações acima, capturamos os backups definitivos em uma
    // única passagem O(linhas ocultadas). Buscar cada backup nas faixas
    // e no vetor de alturas causava custo quadrático em filtros alternados.

    atualizado["_featherOcultas"] = json!(ocultas);
    atualizado["_featherAlturas"] = json!(alturas);
    if registrar {
        let mut tabelas = listar(&w.aba(aba).ok_or(tr!("Aba ausente."))?.sheet)?;
        let i = indice(&tabelas, id).ok_or(tr!("Tabela não encontrada."))?;
        tabelas[i] = atualizado;
        let gravou = salvar_com_historico(w, aba, &tabelas, &area);
        let viva = w.aba_mut(aba).ok_or(tr!("Aba ausente."))?;
        viva.sheet.fechar_transacao();
        viva.versao = viva.versao.wrapping_add(1).max(1);
        gravou?;
    } else {
        mudar(w, aba, id, |t| { *t = atualizado; Ok(()) })?;
    }
    // As alturas mudaram direto na aba: avisa o motor, senão SUBTOTAL e
    // AGREGAR (a linha de total da tabela) continuam somando as linhas
    // que o filtro escondeu.
    w.editar(aba, |_| {});
    Ok(contagem)
}

/// Teto de linhas (escondidas + reexibidas) para o filtro entrar no Ctrl+Z.
const LIMITE_HISTORICO_FILTRO: usize = 200_000;

/// [`limpar_visibilidade`] dentro da transação aberta: cada altura volta
/// com o seu passo de desfazer.
fn restaurar_visibilidade_registrada(w: &mut Workbook, aba: usize, t: &Json) -> Result<(), String> {
    let anteriores = intervalos_lidos(t);
    let backups: std::collections::HashMap<u32,u32> = t["_featherAlturas"].as_array().map(|a| a.iter().filter_map(|v| {
        let i = v.as_array()?;
        Some((u32::try_from(i.first()?.as_u64()?).ok()?,
              u32::try_from(i.get(1)?.as_u64()?).ok()?))
    }).collect()).unwrap_or_default();
    let viva = w.aba_mut(aba).ok_or(tr!("Aba ausente."))?;
    for (a,b) in anteriores {
        for r in a..=b { viva.sheet.registrar_dimensao(false, r, backups.get(&r).copied()); }
    }
    for (&r,&h) in &backups { viva.sheet.registrar_dimensao(false, r, Some(h)); }
    Ok(())
}

pub(in crate::app) fn ordenar(w: &mut Workbook, aba: usize, id: &str, col_rel: u32, asc: bool) -> Result<(), String> {
    let t = sob_id(w, aba, id)?;
    let area = area_de(&t).ok_or(tr!("Área de tabela inválida."))?;
    if col_rel > area.c1 - area.c0 { return Err(tr!("Coluna fora da tabela.").into()); }
    let chave = SortKey { col: area.c0 + col_rel, ascending: asc };
    let max = w.aba(aba).ok_or(tr!("Aba ausente."))?.sheet.max_row;
    let plano = ops::planejar_ordenacao(area.clone(), &[chave], t["temCabecalho"] == true,
        max, |r,c| w.valor(aba,r,c));
    // Ordenar e refiltrar desfazem juntos.
    w.aba_mut(aba).ok_or(tr!("Aba ausente."))?.sheet.abrir_transacao();
    if let Some(plano) = plano {
        w.editar(aba, |sheet| ops::aplicar_ordenacao(sheet, area.c0, area.c1, &plano));
    }
    let nivel = ChaveOrdem { indice: area.c0 + col_rel, ascendente: asc, criterio: CriterioOrdem::Valor };
    let resultado = depois_de_ordenar(w, aba, id, &area, col_rel, asc, &nivel);
    fechar(w, aba);
    resultado
}

/// Grava a ordenação no JSON (o `ordenar` antigo e a `ordenacao` que o
/// `.xlsx` lê) e reaplica os filtros: a ordem das linhas mudou.
fn depois_de_ordenar(
    w: &mut Workbook, aba: usize, id: &str, area: &Rect, col_rel: u32, asc: bool, nivel: &ChaveOrdem,
) -> Result<(), String> {
    let ordenacao = filtro_json::ordenacao_para_json(std::slice::from_ref(nivel), false, area.c0, area.r0);
    mudar(w,aba,id,|t| {
        t["ordenar"]=json!([{"col":col_rel,"desc":!asc}]);
        t["ordenacao"]=ordenacao;
        Ok(())
    })?;
    let atualizado = sob_id(w,aba,id)?;
    if let Some(fs)=atualizado["filtros"].as_array().filter(|x|!x.is_empty()) {
        let col = fs.first().and_then(|f|f["col"].as_u64()).unwrap_or(0) as u32;
        let regra = fs[0]["criterio"].clone();
        filtrar(w,aba,id,col,Some(regra))?;
    }
    Ok(())
}

fn fechar(w: &mut Workbook, aba: usize) {
    if let Some(viva) = w.aba_mut(aba) {
        viva.sheet.fechar_transacao();
        viva.versao = viva.versao.wrapping_add(1).max(1);
    }
}

/// Classifica a tabela pela coluna com um critério de `ordenacao_ext`
/// (cor do preenchimento ou da fonte em cima, lista personalizada), num
/// único Ctrl+Z junto com o refiltro.
pub(in crate::app) fn ordenar_por(
    w: &mut Workbook, aba: usize, id: &str, col_rel: u32, criterio: CriterioOrdem, asc: bool,
) -> Result<bool, String> {
    let t = sob_id(w, aba, id)?;
    let area = area_de(&t).ok_or(tr!("Área de tabela inválida."))?;
    if col_rel > area.c1 - area.c0 { return Err(tr!("Coluna fora da tabela.").into()); }
    let nivel = ChaveOrdem { indice: area.c0 + col_rel, ascendente: asc, criterio };
    let cabecalho = t["temCabecalho"] == true;
    w.aba_mut(aba).ok_or(tr!("Aba ausente."))?.sheet.abrir_transacao();
    let mut mudou = false;
    w.editar(aba, |sheet| {
        mudou = ordenacao_ext::ordenar_por_linhas(sheet, area.clone(), std::slice::from_ref(&nivel), cabecalho, false);
    });
    let resultado = depois_de_ordenar(w, aba, id, &area, col_rel, asc, &nivel);
    fechar(w, aba);
    resultado.map(|_| mudou)
}

/// Reaplicar (Classificar e filtrar ▾ > Reaplicar), como no Excel: depois
/// que os dados mudaram, ordena de novo pela ordenação guardada na tabela
/// (ou no AutoFiltro) e recalcula as linhas escondidas pelos filtros. Um
/// Ctrl+Z desfaz tudo. Devolve (ordenou, linhas escondidas, há filtro).
pub(in crate::app) fn reaplicar(w: &mut Workbook, aba: usize, id: &str) -> Result<(bool, usize, bool), String> {
    let t = sob_id(w, aba, id)?;
    let area = area_de(&t).ok_or(tr!("Área de tabela inválida."))?;
    let cabecalho = t["temCabecalho"] == true;
    let (mut niveis, por_colunas) = if t["ordenacao"].is_object() {
        filtro_json::ordenacao_de_json(&t["ordenacao"], area.c0, area.r0)
    } else { (Vec::new(), false) };
    if niveis.is_empty() {
        // Ordenação antiga (`ordenar`: coluna relativa e se é decrescente).
        niveis = t["ordenar"].as_array().map(|a| a.iter().filter_map(|n| Some(ChaveOrdem {
            indice: area.c0 + u32::try_from(n["col"].as_u64()?).ok()?,
            ascendente: n["desc"] != true,
            criterio: CriterioOrdem::Valor,
        })).collect()).unwrap_or_default();
    }
    let filtros = t["filtros"].as_array().cloned().unwrap_or_default();
    if niveis.is_empty() && filtros.is_empty() {
        return Err(tr!("Não há filtro nem classificação para reaplicar.").into());
    }
    w.aba_mut(aba).ok_or(tr!("Aba ausente."))?.sheet.abrir_transacao();
    let mut ordenou = false;
    if !niveis.is_empty() {
        let limite = if por_colunas { area.r1 } else { area.c1 };
        let niveis: Vec<ChaveOrdem> = niveis.into_iter().filter(|n| n.indice <= limite).collect();
        let maiusculas = t["ordenacao"]["maiusculas"] == true;
        w.editar(aba, |sheet| {
            ordenou = if por_colunas {
                ordenacao_ext::ordenar_por_colunas(sheet, area.clone(), &niveis, false, maiusculas)
            } else {
                ordenacao_ext::ordenar_por_linhas(sheet, area.clone(), &niveis, cabecalho, maiusculas)
            };
        });
    }
    // Recalcula as linhas escondidas com TODOS os critérios atuais: o
    // `filtrar` relê a tabela e aplica cada filtro de novo.
    let resultado = match filtros.first() {
        Some(f) => {
            let col = f["col"].as_u64().unwrap_or(0) as u32;
            filtrar(w, aba, id, col, Some(f["criterio"].clone()))
        }
        None => Ok(0),
    };
    fechar(w, aba);
    resultado.map(|ocultas| (ordenou, ocultas, !filtros.is_empty()))
}

#[cfg(test)]
fn total(w: &Workbook, aba: usize, t: &Json, col: u32) -> Option<f64> {
    let area = area_de(t)?;
    if col<area.c0 || col>area.c1 { return None; }
    let fr = t["totais"].as_array()?.iter().find(|v|v["col"].as_u64() == Some(u64::from(col-area.c0)))?;
    let funcao = fr["fn"].as_str()?;
    let primeira = area.r0 + u32::from(t["temCabecalho"]==true);
    if primeira>area.r1 { return None; }
    let sheet = &w.aba(aba)?.sheet;
    tabela::total_da_coluna(sheet,Rect::new(primeira,col,area.r1,col),funcao,
        |row|sheet.row_heights.get(&row).copied()!=Some(0),
        |r,c|w.valor(aba,r,c))
}

pub(in crate::app) fn crescer(w: &mut Workbook, aba: usize, r: u32, c: u32) -> Result<bool,String> {
    let mut lista = listar(&w.aba(aba).ok_or(tr!("Aba ausente."))?.sheet)?;
    let areas: Vec<AreaDaTabela> = lista.iter().map(|t| AreaDaTabela {
        area: area_de(t).unwrap_or(Rect::cell(r,c)),
        mostrar_totais: t["mostrarTotais"]==true,
        so_filtro: t["soFiltro"]==true || area_de(t).is_none(),
    }).collect();
    let Some((i,nova)) = tabela::crescer_tabela(&areas, r,c) else { return Ok(false); };
    // Não expandir sobre outra tabela.
    if lista.iter().enumerate().any(|(j,t)| i!=j && area_de(t).is_some_and(|a| {
        a.r0<=nova.r1 && nova.r0<=a.r1 && a.c0<=nova.c1 && nova.c0<=a.c1
    })) { return Ok(false); }
    lista[i]["area"] = json!({"r0":nova.r0,"c0":nova.c0,"r1":nova.r1,"c1":nova.c1});
    let nome = lista[i]["nome"].as_str().unwrap_or("").to_string();
    salvar(w,aba,&lista)?;
    if !nome.is_empty() {
        let sheet = w.aba(aba).ok_or(tr!("Aba ausente."))?.sheet.name.clone();
        w.definir_nome(&nome,Rect::na_aba(nova.r0,nova.c0,nova.r1,nova.c1,sheet));
    }
    Ok(true)
}


/// Amplia a tabela de maneira incremental após colar uma MATRIZ de células.
/// A GUI cola milhares de linhas em uma transação; o caminho de digitação
/// `crescer` não pode ser chamado para cada célula, pois releria o JSON N vezes.
/// Só expande se a borda adjacente recebeu conteúdo (não mera formatação).
pub(in crate::app) fn crescer_colagem(
    w: &mut Workbook, aba: usize, colado: Rect,
) -> Result<usize,String> {
    let area=colado.normalized();
    let sheet=&w.aba(aba).ok_or(tr!("Aba ausente."))?.sheet;
    if sheet.tb.is_none() { return Ok(0); }
    let mut lista=listar(sheet)?;
    let mut mudadas=0;
    let mut atualizacoes=Vec::new();
    for i in 0..lista.len() {
        if lista[i]["soFiltro"]==true || lista[i]["mostrarTotais"]==true {continue;}
        let Some(mut a)=area_de(&lista[i]) else {continue};
        let mut aumentou=false;
        // Colagem em linhas imediatamente abaixo, iniciando em qualquer
        // coluna existente da tabela e sem ultrapassar outras tabelas.
        if a.r1.checked_add(1).is_some_and(|proxima|area.r0<=proxima && area.r1>=proxima) {
            let c0=area.c0.max(a.c0);
            let c1=area.c1.min(a.c1);
            if c0<=c1 && (c0..=c1).any(|c|!sheet.get_raw(a.r1+1,c).is_empty()) {
                let nova=Rect::new(a.r0,a.c0,a.r1.max(area.r1),a.c1);
                let colide=lista.iter().enumerate().any(|(j,t)|i!=j && area_de(t).is_some_and(|b|
                    nova.r0<=b.r1 && b.r0<=nova.r1 && nova.c0<=b.c1 && b.c0<=nova.c1));
                if !colide { a=nova; aumentou=true; }
            }
        }
        // Colagem na coluna imediatamente à direita, com altura sobreposta.
        if a.c1.checked_add(1).is_some_and(|proxima|area.c0<=proxima && area.c1>=proxima) {
            let r0=area.r0.max(a.r0);
            let r1=area.r1.min(a.r1);
            if r0<=r1 && (r0..=r1).take(4096).any(|r|!sheet.get_raw(r,a.c1+1).is_empty()) {
                let nova=Rect::new(a.r0,a.c0,a.r1,area.c1.max(a.c1));
                let colide=lista.iter().enumerate().any(|(j,t)|i!=j && area_de(t).is_some_and(|b|
                    nova.r0<=b.r1 && b.r0<=nova.r1 && nova.c0<=b.c1 && b.c0<=nova.c1));
                if !colide {a=nova;aumentou=true;}
            }
        }
        if aumentou {
            lista[i]["area"]=json!({"r0":a.r0,"c0":a.c0,"r1":a.r1,"c1":a.c1});
            if let Some(nome)=lista[i]["nome"].as_str(){
                atualizacoes.push((nome.to_owned(),a));
            }
            mudadas+=1;
        }
    }
    if mudadas==0 {return Ok(0);}
    salvar(w,aba,&lista)?;
    let nome_aba=w.aba(aba).ok_or(tr!("Aba ausente."))?.sheet.name.clone();
    for (nome,rect) in atualizacoes {
        w.definir_nome(&nome,Rect::na_aba(rect.r0,rect.c0,rect.r1,rect.c1,nome_aba.clone()));
    }
    Ok(mudadas)
}

/// Consulta incremental dos valores distintos da coluna, sem materializar
/// todas as linhas da planilha. Cada chamada examina no máximo `linhas` linhas
/// (não valores únicos); `vistos` evita repetições entre lotes. A GUI mantém
/// um cursor para continuar até o fim quando a pessoa solicitar.
pub(in crate::app) fn distintos_lote(
    w: &Workbook, aba: usize, t: &Json, col: u32,
    cursor: u32, linhas: u32, vistos: &mut HashSet<String>,
) -> Result<(Vec<String>, u32, u32), String> {
    let area = area_de(t).ok_or(tr!("Área inválida."))?;
    if col < area.c0 || col > area.c1 {
        return Err(tr!("Coluna fora da tabela.").into());
    }
    let sheet = &w.aba(aba).ok_or(tr!("Aba ausente."))?.sheet;
    let inicio = area.r0.saturating_add(u32::from(t["temCabecalho"] == true));
    let ultimo = area.r1.min(sheet.max_row);
    let proxima = cursor.max(inicio);
    if proxima > ultimo {
        return Ok((Vec::new(), proxima, ultimo));
    }
    let fim = proxima.saturating_add(linhas.max(1) - 1).min(ultimo);
    let mut valores = Vec::new();
    for row in proxima..=fim {
        let texto = match w.valor(aba, row, col) {
            Valor::Empty => "(Vazias)".to_owned(),
            outro => filtro::texto_js(&outro),
        };
        if vistos.insert(texto.clone()) {
            valores.push(texto);
        }
    }
    Ok((valores, fim.saturating_add(1), ultimo))
}

/// Ajusta tabela, filtros e totais após inserir/excluir linhas ou colunas.
/// Usa o reflow do motor Rust (mesma semântica do ajustarRect TS) e não move
/// nenhum dado: o Workbook já fez a mutação colunar.
pub(in crate::app) fn refluir(
    w: &mut Workbook, aba: usize, coluna: bool, at: u32, count: u32, removendo: bool
) -> Result<(), String> {
    use feathersheet_core::operacoes::reflow::ajustar_intervalo_tabela;
    if count == 0 {return Ok(());}
    let lista_antiga = listar(&w.aba(aba).ok_or(tr!("Aba ausente."))?.sheet)?;
    let mut lista_nova = Vec::with_capacity(lista_antiga.len());
    let mut apagadas = Vec::new();
    let delta = if removendo { -(count as i64) } else { count as i64 };
    for mut t in lista_antiga {
        let Some(area) = area_de(&t) else {lista_nova.push(t);continue;};
        let (inicio,fim) = if coluna {(area.c0,area.c1)} else {(area.r0,area.r1)};
        let Some((a,b))=ajustar_intervalo_tabela(inicio as i64,fim as i64,at as i64,delta)
            else {
                if let Some(nome)=t["nome"].as_str(){apagadas.push(nome.to_string());}
                continue;
            };
        let mut area_nova=area.clone();
        if coluna {
            area_nova.c0=a as u32;
            area_nova.c1=b as u32;
            // Critérios de filtro e de total referem-se a colunas RELATIVAS.
            let mut filtros=t["filtros"].as_array().cloned().unwrap_or_default();
            filtros.retain_mut(|f| {
                let Some(rel)=f["col"].as_u64() else {return false};
                let abs=area.c0 as i64+rel as i64;
                // Mantém critério junto à coluna deslocada.
                let nova= if removendo && abs>=at as i64 && abs<at as i64+count as i64 {
                    return false;
                } else if abs>=at as i64 {abs+delta} else {abs};
                f["col"]=json!(nova-a);
                true
            });
            t["filtros"]=json!(filtros);
            let mut totais=t["totais"].as_array().cloned().unwrap_or_default();
            totais.retain_mut(|v| {
                let Some(rel)=v["col"].as_u64() else {return false};
                let abs=area.c0 as i64+rel as i64;
                let nova=if removendo && abs>=at as i64 && abs<at as i64+count as i64 {
                    return false;
                } else if abs>=at as i64 {abs+delta} else {abs};
                v["col"]=json!(nova-a);
                true
            });
            t["totais"]=json!(totais);
        } else {
            area_nova.r0=a as u32;
            area_nova.r1=b as u32;
            // Os intervalos de linhas ocultas são absolutos e acompanham a
            // estrutura; para não deixar linhas invisíveis presas após excluir.
            let antigos=intervalos_lidos(&t);
            let mut novos=Vec::new();
            for (r0,r1) in antigos {
                if let Some((nr0,nr1))=ajustar_intervalo_tabela(r0 as i64,r1 as i64,at as i64,delta) {
                    novos.push((nr0 as u32,nr1 as u32));
                }
            }
            t["_featherOcultas"]=json!(novos);
            if let Some(antigas)=t["_featherAlturas"].as_array() {
                let mut novos=Vec::new();
                for valor in antigas {
                    let Some(r)=valor.get(0).and_then(Json::as_u64) else {continue};
                    let Some(h)=valor.get(1).and_then(Json::as_u64) else {continue};
                    let rr=r as i64;
                    let novo=if removendo && rr>=at as i64 && rr<at as i64+count as i64 {
                        continue;
                    } else if rr>=at as i64 {rr+delta} else {rr};
                    if novo>=0 {novos.push(json!([novo,h]));}
                }
                t["_featherAlturas"]=json!(novos);
            }
        }
        t["area"]=json!({"r0":area_nova.r0,"c0":area_nova.c0,
            "r1":area_nova.r1,"c1":area_nova.c1});
        lista_nova.push(t);
    }
    salvar(w,aba,&lista_nova)?;
    for nome in apagadas { w.remover_nome(&nome); }
    let nome_aba=w.aba(aba).ok_or(tr!("Aba ausente."))?.sheet.name.clone();
    for t in &lista_nova {
        if t["soFiltro"]==true {continue;}
        if let (Some(nome),Some(area))=(t["nome"].as_str(),area_de(t)) {
            w.definir_nome(nome,Rect::na_aba(area.r0,area.c0,area.r1,area.c1,nome_aba.clone()));
        }
    }
    // Reavalia o predicado na nova posição das linhas, sem alterar cabeçalhos.
    for t in &lista_nova {
        let Some(id)=t["id"].as_str() else {continue};
        let Some(f)=t["filtros"].as_array().and_then(|v|v.first()) else {continue};
        if let Some(col)=f["col"].as_u64() {
            let _=filtrar(w,aba,id,col as u32,Some(f["criterio"].clone()))?;
        }
    }
    Ok(())
}


/// Ctrl+Shift+L: AutoFiltro sobre região atual. Se já existir um AutoFiltro
/// sem critérios, desativa-o; com critérios, remove critérios e alturas.
/// Uma tabela verdadeira mantém seu objeto e usa o painel de filtros.
pub(in crate::app) fn alternar_auto_filtro(w: &mut Workbook, aba: usize, selecao: Rect, r:u32, c:u32) -> Result<String,String> {
    let mut lista=listar(&w.aba(aba).ok_or(tr!("Aba ausente."))?.sheet)?;
    let encontrado=lista.iter().position(|t|area_de(t).is_some_and(|a|a.contains(r,c)));
    if let Some(i)=encontrado {
        if lista[i]["soFiltro"]!=true {
            return Ok(tr!("Use o botão do cabeçalho ou Design da tabela para filtrar.").into());
        }
        let id=lista[i]["id"].as_str().unwrap_or("").to_string();
        let atual=lista[i]["filtros"].as_array().map(|a|!a.is_empty()).unwrap_or(false);
        if atual {
            // Um único restauro: evitar reavaliar toda a tabela uma vez
            // por coluna filtrada (custoso com milhões de registros).
            limpar_todos_filtros(w,aba,&id)?;
            return Ok(tr!("Critérios de AutoFiltro removidos.").into());
        }
        let antiga=lista.remove(i);
        limpar_visibilidade(w,aba,&antiga)?;
        let area=area_de(&antiga).ok_or(tr!("Área de AutoFiltro inválida."))?;
        salvar_com_historico(w,aba,&lista,&area)?;
        return Ok(tr!("AutoFiltro desativado.").into());
    }
    let sheet=&w.aba(aba).ok_or(tr!("Aba ausente."))?.sheet;
    let inicio=selecao.normalized();
    let area=if inicio.r0==inicio.r1 && inicio.c0==inicio.c1 {
        ops::regiao_atual(sheet,inicio)
    } else {inicio};
    let area=area_valida(area)?;
    if sheet.get_raw(area.r0,area.c0).is_empty() && area.r0==area.r1 && area.c0==area.c1 {
        return Err(tr!("Não há dados para AutoFiltro.").into());
    }
    if lista.iter().filter_map(area_de).any(|a|a.r0<=area.r1 && area.r0<=a.r1
        && a.c0<=area.c1 && area.c0<=a.c1) {
        return Err(tr!("O AutoFiltro não pode sobrepor outra tabela.").into());
    }
    let id=format!("flt_{}_{}_{}",aba,area.r0,area.c0);
    lista.push(json!({"id":id,"nome":"Filtro","area":area,"temCabecalho":true,
        "faixas":false,"soFiltro":true,"filtros":[]}));
    salvar_com_historico(w,aba,&lista,&area)?;
    Ok(tr!("AutoFiltro ativado. Use as setas do cabeçalho.").into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn autofiltro_sem_criterios_usa_historico_compacto() {
        let mut w=Workbook::new();
        w.escrever_celula(0,0,0,"Valor".into());
        w.escrever_celula(0,1,0,"100".into());
        alternar_auto_filtro(&mut w,0,Rect::new(0,0,1,0),0,0).unwrap();
        assert_eq!(listar(&w.aba(0).unwrap().sheet).unwrap().len(),1);
        assert!(w.aba_mut(0).unwrap().sheet.undo());
        assert!(listar(&w.aba(0).unwrap().sheet).unwrap().is_empty());
        assert_eq!(w.aba(0).unwrap().sheet.get_raw(1,0),"100");
        assert!(w.aba_mut(0).unwrap().sheet.redo());
        assert_eq!(listar(&w.aba(0).unwrap().sheet).unwrap().len(),1);
        alternar_auto_filtro(&mut w,0,Rect::cell(0,0),0,0).unwrap();
        assert!(listar(&w.aba(0).unwrap().sheet).unwrap().is_empty());
        assert!(w.aba_mut(0).unwrap().sheet.undo());
        assert_eq!(listar(&w.aba(0).unwrap().sheet).unwrap().len(),1);
    }

    #[test]
    fn alterar_estilo_e_total_sao_desfeitos_sem_alterar_celulas() {
        let mut w=Workbook::new();
        w.escrever_celula(0,0,0,"Produto".into());
        w.escrever_celula(0,1,0,"Caneta".into());
        super::super::criar(&mut w,0,Rect::new(0,0,1,0),"Tabela1",true,false).unwrap();
        let id="tab_Tabela1";
        estilo(&mut w,0,id,Estilo::Tema("verde".into())).unwrap();
        estilo(&mut w,0,id,Estilo::FaixasColuna(true)).unwrap();
        estilo(&mut w,0,id,Estilo::Totais(true)).unwrap();
        assert_eq!(sob_id(&w,0,id).unwrap()["mostrarTotais"],true);
        assert_eq!(sob_id(&w,0,id).unwrap()["estilo"]["faixasColuna"],true);
        assert!(w.aba_mut(0).unwrap().sheet.undo());
        assert_ne!(sob_id(&w,0,id).unwrap()["mostrarTotais"],true);
        assert!(w.aba_mut(0).unwrap().sheet.undo());
        assert_ne!(sob_id(&w,0,id).unwrap()["estilo"]["faixasColuna"],true);
        assert!(w.aba_mut(0).unwrap().sheet.undo());
        assert_eq!(sob_id(&w,0,id).unwrap()["estilo"]["tema"],"azul");
        assert_eq!(w.aba(0).unwrap().sheet.get_raw(1,0),"Caneta");
        assert!(w.aba_mut(0).unwrap().sheet.redo());
        assert!(w.aba_mut(0).unwrap().sheet.redo());
        assert!(w.aba_mut(0).unwrap().sheet.redo());
        assert_eq!(sob_id(&w,0,id).unwrap()["estilo"]["tema"],"verde");
        assert_eq!(sob_id(&w,0,id).unwrap()["estilo"]["faixasColuna"],true);
        assert_eq!(sob_id(&w,0,id).unwrap()["mostrarTotais"],true);
    }

    #[test]
    fn alterar_estilo_invalido_nao_adiciona_undo_ou_muda_metadados() {
        let mut w=Workbook::new();
        super::super::criar(&mut w,0,Rect::new(0,0,2,1),"Tabela1",true,false).unwrap();
        let antes=sob_id(&w,0,"tab_Tabela1").unwrap();
        let historico=w.aba(0).unwrap().sheet.can_undo();
        assert!(estilo(&mut w,0,"tab_Tabela1",Estilo::Tema("sem-tema".into())).is_err());
        assert_eq!(sob_id(&w,0,"tab_Tabela1").unwrap(),antes);
        assert_eq!(w.aba(0).unwrap().sheet.can_undo(),historico);
    }

    #[test]
    fn redimensionamento_preserva_celulas_referencias_e_evitar_sobreposicao() {
        let mut w=Workbook::new();
        w.escrever_celula(0,0,0,"Nome".into());
        w.escrever_celula(0,0,1,"Valor".into());
        w.escrever_celula(0,1,0,"Caneta".into());
        super::super::criar(&mut w,0,Rect::new(0,0,1,1),"Tabela1",true,false).unwrap();
        super::super::criar(&mut w,0,Rect::new(0,4,1,4),"Tabela2",true,false).unwrap();
        redimensionar(&mut w,0,"tab_Tabela1",Rect::new(0,0,4,3)).unwrap();
        assert_eq!(sob_id(&w,0,"tab_Tabela1").unwrap()["area"]["c1"],3);
        assert_eq!(w.nomeados().iter().find(|v|v.nome=="Tabela1").unwrap().area.r1,4);
        assert_eq!(w.aba(0).unwrap().sheet.get_raw(1,0),"Caneta");
        assert!(redimensionar(&mut w,0,"tab_Tabela1",Rect::new(0,0,4,4)).is_err());
        assert!(redimensionar(&mut w,0,"tab_Tabela1",Rect::new(1,0,4,2)).is_err());
        assert_eq!(sob_id(&w,0,"tab_Tabela1").unwrap()["area"]["c1"],3);
    }

    #[test]
    fn redimensionamento_restaura_alturas_de_linhas_ocultas_antigas() {
        let mut w=Workbook::new();
        for (r,v) in ["Categoria","A","B","C"].iter().enumerate(){
            w.escrever_celula(0,r as u32,0,(*v).into());
        }
        super::super::criar(&mut w,0,Rect::new(0,0,3,0),"Tabela1",true,false).unwrap();
        w.aba_mut(0).unwrap().sheet.row_heights.insert(2,32);
        filtrar(&mut w,0,"tab_Tabela1",0,Some(json!(["A"]))).unwrap();
        assert_eq!(w.aba(0).unwrap().sheet.row_heights.get(&2),Some(&0));
        redimensionar(&mut w,0,"tab_Tabela1",Rect::new(0,0,1,0)).unwrap();
        assert_eq!(w.aba(0).unwrap().sheet.row_heights.get(&2),Some(&32));
        assert_eq!(w.aba(0).unwrap().sheet.get_raw(2,0),"B");
    }

    #[test]
    fn altera_nome_estilo_e_converte_sem_apagar_celulas() {
        let mut w=Workbook::new();
        w.escrever_celula(0,0,0,"Produto".into());
        w.escrever_celula(0,1,0,"Caneta".into());
        super::super::criar(&mut w,0,Rect::new(0,0,1,0),"Tabela1",true,false).unwrap();
        renomear(&mut w,0,"tab_Tabela1","Estoque").unwrap();
        estilo(&mut w,0,"tab_Tabela1",Estilo::Tema("verde".into())).unwrap();
        estilo(&mut w,0,"tab_Tabela1",Estilo::FaixasColuna(true)).unwrap();
        let t=sob_id(&w,0,"tab_Tabela1").unwrap();
        assert_eq!(t["nome"],"Estoque");
        assert_eq!(t["estilo"]["tema"],"verde");
        converter(&mut w,0,"tab_Tabela1").unwrap();
        assert_eq!(w.aba(0).unwrap().sheet.get_raw(1,0),"Caneta");
        assert_eq!(listar(&w.aba(0).unwrap().sheet).unwrap().len(),0);
    }

    #[test]
    fn filtros_escondem_linhas_sem_apagar_valores_e_limpar_recupera_alturas() {
        let mut w=Workbook::new();
        for (r,valor) in ["Tipo","A","B","A","C"].iter().enumerate(){
            w.escrever_celula(0,r as u32,0,(*valor).into());
        }
        w.aba_mut(0).unwrap().sheet.row_heights.insert(2, 31);
        super::super::criar(&mut w,0,Rect::new(0,0,4,0),"Tabela1",true,false).unwrap();
        let n=filtrar(&mut w,0,"tab_Tabela1",0,Some(json!({"op":"equals_text","val":"A"}))).unwrap();
        assert_eq!(n,2);
        assert_eq!(w.aba(0).unwrap().sheet.row_heights.get(&2),Some(&0));
        assert_eq!(w.aba(0).unwrap().sheet.get_raw(2,0),"B");
        filtrar(&mut w,0,"tab_Tabela1",0,None).unwrap();
        assert_eq!(w.aba(0).unwrap().sheet.row_heights.get(&2),Some(&31));
        assert!(!w.aba(0).unwrap().sheet.row_heights.contains_key(&4));
    }
    #[test]
    fn limpar_filtros_combinados_nao_reexibe_ocultas_manuais() {
        let mut w=Workbook::new();
        for (r,a,b) in [
            (0,"Tipo","Numero"),(1,"A","10"),(2,"B","20"),
            (3,"A","30"),(4,"C","10"),
        ] {
            w.escrever_celula(0,r,0,a.into());
            w.escrever_celula(0,r,1,b.into());
        }
        super::super::criar(&mut w,0,Rect::new(0,0,4,1),"Tabela1",true,false).unwrap();
        w.aba_mut(0).unwrap().sheet.row_heights.insert(2,0);
        w.aba_mut(0).unwrap().sheet.row_heights.insert(4,29);
        filtrar(&mut w,0,"tab_Tabela1",0,Some(json!(["A"]))).unwrap();
        filtrar(&mut w,0,"tab_Tabela1",1,Some(json!({"op":"gt","val":15}))).unwrap();
        assert_eq!(sob_id(&w,0,"tab_Tabela1").unwrap()["filtros"].as_array().unwrap().len(),2);
        assert_eq!(limpar_todos_filtros(&mut w,0,"tab_Tabela1").unwrap(),2);
        assert_eq!(w.aba(0).unwrap().sheet.row_heights.get(&2),Some(&0));
        assert_eq!(w.aba(0).unwrap().sheet.row_heights.get(&4),Some(&29));
        assert!(!w.aba(0).unwrap().sheet.row_heights.contains_key(&1));
        assert_eq!(w.aba(0).unwrap().sheet.get_raw(2,0),"B");
        assert_eq!(limpar_todos_filtros(&mut w,0,"tab_Tabela1").unwrap(),0);
    }

    #[test]
    fn criterio_vazio_gravado_em_fshz_e_limpeza_preservam_valores() {
        let mut w=Workbook::new();
        w.escrever_celula(0,0,0,"Item".into());
        w.escrever_celula(0,1,0,"A".into());
        w.escrever_celula(0,3,0,"C".into());
        super::super::criar(&mut w,0,Rect::new(0,0,3,0),"Tabela1",true,false).unwrap();
        assert_eq!(filtrar(&mut w,0,"tab_Tabela1",0,
            Some(json!({"op":"is_blank","val":null}))).unwrap(),2);
        assert_eq!(w.aba(0).unwrap().sheet.row_heights.get(&1),Some(&0));
        assert_ne!(w.aba(0).unwrap().sheet.row_heights.get(&2),Some(&0));
        let bytes=feathersheet_io::da_workbook::pasta_gravar_bytes(&w,0).unwrap();
        let outra=feathersheet_io::para_workbook::para_workbook(
            &feathersheet_io::fshz_read::ler_fshz(&bytes).unwrap());
        assert_eq!(sob_id(&outra,0,"tab_Tabela1").unwrap()["filtros"][0]["criterio"]["op"],"is_blank");
        assert_eq!(filtrar(&mut w,0,"tab_Tabela1",0,
            Some(json!({"op":"is_not_blank","val":null}))).unwrap(),1);
        assert_eq!(w.aba(0).unwrap().sheet.row_heights.get(&2),Some(&0));
        limpar_todos_filtros(&mut w,0,"tab_Tabela1").unwrap();
        assert!(!w.aba(0).unwrap().sheet.row_heights.contains_key(&2));
        assert_eq!(w.aba(0).unwrap().sheet.get_raw(3,0),"C");
    }

    #[test]
    fn milhares_de_linhas_filtradas_podem_trocar_criterio_sem_perder_alturas() {
        let mut w=Workbook::new();
        w.escrever_celula(0,0,0,"Tipo".into());
        // Alternar A/B produz muitas faixas de linhas ocultas; o fluxo
        // nao pode comparar cada backup com todas as faixas novamente.
        for r in 1..=4096 {
            w.escrever_celula(0,r,0,if r%2==0 {"B"} else {"A"}.into());
        }
        super::super::criar(&mut w,0,Rect::new(0,0,4096,0),
            "TabelaGrande",true,false).unwrap();
        w.aba_mut(0).unwrap().sheet.row_heights.insert(100,0);
        w.aba_mut(0).unwrap().sheet.row_heights.insert(99,37);
        assert_eq!(filtrar(&mut w,0,"tab_TabelaGrande",0,Some(json!(["A"]))).unwrap(),2048);
        assert_eq!(filtrar(&mut w,0,"tab_TabelaGrande",0,Some(json!(["B"]))).unwrap(),2048);
        assert_eq!(filtrar(&mut w,0,"tab_TabelaGrande",0,Some(json!(["A"]))).unwrap(),2048);
        assert_eq!(limpar_todos_filtros(&mut w,0,"tab_TabelaGrande").unwrap(),1);
        assert_eq!(w.aba(0).unwrap().sheet.row_heights.get(&100),Some(&0));
        assert_eq!(w.aba(0).unwrap().sheet.row_heights.get(&99),Some(&37));
        assert!(!w.aba(0).unwrap().sheet.row_heights.contains_key(&101));
        assert_eq!(w.aba(0).unwrap().sheet.get_raw(4096,0),"B");
    }

    #[test]
    fn autofiltro_multiplas_colunas_limpa_todos_criterios_em_uma_operacao() {
        let mut w=Workbook::new();
        for (r,a,b) in [(0,"Tipo","Valor"),(1,"A","10"),(2,"B","20"),(3,"A","30")] {
            w.escrever_celula(0,r,0,a.into());
            w.escrever_celula(0,r,1,b.into());
        }
        let id="flt_0_0_0";
        alternar_auto_filtro(&mut w,0,Rect::new(0,0,3,1),0,0).unwrap();
        w.aba_mut(0).unwrap().sheet.row_heights.insert(2,0);
        filtrar(&mut w,0,id,0,Some(json!(["A"]))).unwrap();
        filtrar(&mut w,0,id,1,Some(json!({"op":"gt","val":15}))).unwrap();
        assert_eq!(sob_id(&w,0,id).unwrap()["filtros"].as_array().unwrap().len(),2);
        let status=alternar_auto_filtro(&mut w,0,Rect::cell(0,0),0,0).unwrap();
        assert!(status.contains("removidos"));
        assert!(sob_id(&w,0,id).unwrap()["filtros"].as_array().unwrap().is_empty());
        assert_eq!(w.aba(0).unwrap().sheet.row_heights.get(&2),Some(&0));
        assert!(!w.aba(0).unwrap().sheet.row_heights.contains_key(&1));
        assert_eq!(w.aba(0).unwrap().sheet.get_raw(2,1),"20");
        // Outra ativacao desliga o AutoFiltro vazio, mas preserva os valores.
        assert!(alternar_auto_filtro(&mut w,0,Rect::cell(0,0),0,0)
            .unwrap().contains("desativado"));
        assert!(listar(&w.aba(0).unwrap().sheet).unwrap().is_empty());
    }

    #[test]
    fn ordenar_soma_visivel_e_expande_tabela() {
        let mut w=Workbook::new();
        for (r,s) in ["Valor","3","1","2"].iter().enumerate(){
            w.escrever_celula(0,r as u32,0,(*s).into());
        }
        super::super::criar(&mut w,0,Rect::new(0,0,3,0),"Tabela1",true,false).unwrap();
        estilo(&mut w,0,"tab_Tabela1",Estilo::TotalColuna{col:0,funcao:"soma".into()}).unwrap();
        assert_eq!(total(&w,0,&sob_id(&w,0,"tab_Tabela1").unwrap(),0),Some(6.0));
        ordenar(&mut w,0,"tab_Tabela1",0,true).unwrap();
        assert_eq!(w.aba(0).unwrap().sheet.get_raw(1,0),"1");
        assert!(crescer(&mut w,0,4,0).unwrap());
        assert_eq!(sob_id(&w,0,"tab_Tabela1").unwrap()["area"]["r1"],4);
    }
    #[test]
    fn reaplicar_refaz_ordem_e_filtro_depois_de_editar() {
        let mut w=Workbook::new();
        for (r,(tipo,valor)) in [("Tipo","Valor"),("A","3"),("B","1"),("A","2")].iter().enumerate() {
            w.escrever_celula(0,r as u32,0,(*tipo).into());
            w.escrever_celula(0,r as u32,1,(*valor).into());
        }
        alternar_auto_filtro(&mut w,0,Rect::cell(0,0),0,0).unwrap();
        let id="flt_0_0_0";
        assert!(reaplicar(&mut w,0,id).is_err(), "sem filtro nem ordem não há o que reaplicar");
        ordenar(&mut w,0,id,1,true).unwrap();
        assert_eq!(filtrar(&mut w,0,id,0,Some(json!(["A"]))).unwrap(),1);
        // Os dados mudam: B passa a A e um valor fica fora de ordem.
        w.escrever_celula(0,1,0,"A".into());
        w.escrever_celula(0,3,1,"0".into());
        let (ordenou,ocultas,filtro)=reaplicar(&mut w,0,id).unwrap();
        assert!(ordenou && filtro);
        assert_eq!(ocultas,0, "todas as linhas agora são A");
        let s=&w.aba(0).unwrap().sheet;
        let valores:Vec<String>=(1..=3).map(|r| s.get_raw(r,1)).collect();
        assert_eq!(valores,["0","1","2"]);
        assert_eq!(s.row_heights.get(&1),None);
        // Um Ctrl+Z desfaz a ordem e o filtro juntos.
        assert!(w.aba_mut(0).unwrap().sheet.undo());
        let s=&w.aba(0).unwrap().sheet;
        assert_eq!(s.get_raw(3,1),"0");
    }
    #[test]
    fn autofiltro_reflow_e_fshz_preservam_intervalo() {
        let mut w=Workbook::new();
        for (r,v) in ["Tipo","A","B","A"].iter().enumerate() {
            w.escrever_celula(0,r as u32,0,(*v).into());
        }
        let ativado=alternar_auto_filtro(&mut w,0,Rect::cell(0,0),0,0).unwrap();
        assert!(ativado.contains("ativado"));
        let id="flt_0_0_0";
        assert_eq!(sob_id(&w,0,id).unwrap()["soFiltro"],true);
        let n=filtrar(&mut w,0,id,0,Some(json!(["A"]))).unwrap();
        assert_eq!(n,1);
        assert_eq!(w.aba(0).unwrap().sheet.row_heights.get(&2),Some(&0));
        let bytes=feathersheet_io::da_workbook::pasta_gravar_bytes(&w,0).unwrap();
        let reaberta=feathersheet_io::para_workbook::para_workbook(
            &feathersheet_io::fshz_read::ler_fshz(&bytes).unwrap());
        assert_eq!(sob_id(&reaberta,0,id).unwrap()["filtros"][0]["col"],0);
        filtrar(&mut w,0,id,0,None).unwrap();
        let desativado=alternar_auto_filtro(&mut w,0,Rect::cell(0,0),0,0).unwrap();
        assert!(desativado.contains("desativado"));
        assert!(listar(&w.aba(0).unwrap().sheet).unwrap().is_empty());
        super::super::criar(&mut w,0,Rect::new(0,0,3,0),"Tabela1",true,false).unwrap();
        estilo(&mut w,0,"tab_Tabela1",Estilo::TotalColuna{
            col:0,funcao:"contagem".into()
        }).unwrap();
        w.inserir_linhas(0,2,1);
        refluir(&mut w,0,false,2,1,false).unwrap();
        let tabela=sob_id(&w,0,"tab_Tabela1").unwrap();
        assert_eq!(tabela["area"]["r1"],4);
        assert_eq!(tabela["totais"][0]["fn"],"contagem");
        assert_eq!(w.aba(0).unwrap().sheet.get_raw(3,0),"B");
    }

    #[test]
    fn colagem_em_lote_expande_tabela_sem_engolir_vizinha() {
        let mut w=Workbook::new();
        w.escrever_celula(0,0,0,"Produto".into());
        w.escrever_celula(0,1,0,"Caneta".into());
        super::super::criar(&mut w,0,Rect::new(0,0,1,0),"Tabela1",true,false).unwrap();
        w.escrever_celula(0,2,0,"Caderno".into());
        w.escrever_celula(0,3,0,"Borracha".into());
        assert_eq!(crescer_colagem(&mut w,0,Rect::new(2,0,3,0)).unwrap(),1);
        let t=sob_id(&w,0,"tab_Tabela1").unwrap();
        assert_eq!(t["area"]["r1"],3);
        assert_eq!(w.nomeados().iter().find(|n|n.nome=="Tabela1").unwrap().area.r1,3);
        // Metadados mudam; célula original e dados colados continuam no motor.
        assert_eq!(w.aba(0).unwrap().sheet.get_raw(1,0),"Caneta");
        assert_eq!(w.aba(0).unwrap().sheet.get_raw(3,0),"Borracha");
        assert_eq!(crescer_colagem(&mut w,0,Rect::new(2,0,3,0)).unwrap(),0);
        super::super::criar(&mut w,0,Rect::new(5,0,6,0),"Tabela2",true,false).unwrap();
        w.escrever_celula(0,4,0,"Divisa".into());
        w.escrever_celula(0,5,0,"Outra".into());
        assert_eq!(crescer_colagem(&mut w,0,Rect::new(4,0,5,0)).unwrap(),0);
        assert_eq!(sob_id(&w,0,"tab_Tabela1").unwrap()["area"]["r1"],3);
    }

    #[test]
    fn distintos_lote_continua_apos_o_primeiro_bloco_sem_duplicar() {
        let mut w = Workbook::new();
        w.escrever_celula(0, 0, 0, "Produto".into());
        w.escrever_celula(0, 1, 0, "A".into());
        w.escrever_celula(0, 2, 0, "A".into());
        w.escrever_celula(0, 4, 0, "B".into());
        let tabela = json!({
            "area": {"r0":0,"r1":4,"c0":0,"c1":0},
            "temCabecalho": true
        });
        let mut vistos = HashSet::new();
        let (primeiro, cursor, ultimo) =
            distintos_lote(&w, 0, &tabela, 0, 1, 2, &mut vistos).unwrap();
        assert_eq!(primeiro, vec!["A"]);
        assert_eq!((cursor, ultimo), (3, 4));
        let (segundo, cursor, _) =
            distintos_lote(&w, 0, &tabela, 0, cursor, 1, &mut vistos).unwrap();
        assert_eq!(segundo, vec!["(Vazias)"]);
        let (terceiro, cursor, _) =
            distintos_lote(&w, 0, &tabela, 0, cursor, 100, &mut vistos).unwrap();
        assert_eq!(terceiro, vec!["B"]);
        assert_eq!(cursor, 5);
        assert!(distintos_lote(&w, 0, &tabela, 0, cursor, 100, &mut vistos)
            .unwrap().0.is_empty());
        assert_eq!(vistos.len(), 3);
    }

}
