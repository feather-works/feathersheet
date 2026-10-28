//! Edição e atualização de dinâmicas persistidas na aba.
use super::*;
use feathersheet_core::dinamica::dinamica_layout::Resultado;
use serde_json::{json, Value as Json};

/// Lê índices do legado TypeScript: -1 = primeira coluna calculada.
fn indice(v: &Json, origem: &Rect, n_calculados: usize) -> Option<usize> {
    let inteiro = v.as_i64()?;
    let valor = if inteiro < 0 {
        let relativo = usize::try_from(-inteiro - 1).ok()?;
        if relativo >= n_calculados { return None; }
        origem.c1 as usize + 1 + relativo
    } else {
        usize::try_from(inteiro).ok()?
    };
    (valor >= origem.c0 as usize && valor <= origem.c1 as usize + n_calculados).then_some(valor)
}

fn lista_indices(v: &Json, origem: &Rect, n: usize) -> Option<Vec<usize>> {
    v.as_array()?.iter().map(|item| indice(item, origem, n)).collect()
}

pub(super) fn agregacao(como: &str) -> Option<AgregacaoOp> {
    Some(match como.to_ascii_lowercase().as_str() {
        "soma" | "sum" => AgregacaoOp::Soma,
        "contagem" | "conta" | "contar" | "count" | "contarvalores" => AgregacaoOp::Contagem,
        "media" | "média" | "avg" | "average" => AgregacaoOp::Media,
        "min" | "mín" | "minimo" | "mínimo" => AgregacaoOp::Min,
        "max" | "máx" | "maximo" | "máximo" => AgregacaoOp::Max,
        "desviopadrao" | "desvio_padrao" | "desvio" | "stdev" => AgregacaoOp::DesvioPadrao,
        "moda" | "mode" => AgregacaoOp::Moda,
        _ => return None,
    })
}

pub(super) fn ler_estado(app: &App, destino: usize, meta: &Json) -> Result<Estado, String> {
    let origem = retangulo_json(&meta["fonte"]["area"])
        .ok_or(tr!("Intervalo de origem ausente ou inválido."))?;
    if origem.r1 <= origem.r0 {
        return Err(tr!("Origem sem dados abaixo do cabeçalho.").into());
    }
    let fonte = meta["fonte"]["aba"].as_str()
        .or_else(|| app.workbook.aba(destino).map(|a| a.sheet.name.as_str()))
        .ok_or(tr!("Aba de origem ausente."))?;
    let folha_origem = app.workbook.nomes().iter()
        .position(|nome| nome == fonte)
        .ok_or_else(|| trf!("A aba de origem '{fonte}' foi removida."))?;
    let id = meta["id"].as_str().ok_or(tr!("ID da dinâmica ausente."))?.to_owned();
    let calculados: Vec<Calculado> = match meta["calculados"].as_array() {
        Some(lista) => lista.iter().map(|v| {
            Some(Calculado { nome: v["nome"].as_str()?.to_owned(),
                formula: v["formula"].as_str()?.to_owned() })
        }).collect::<Option<Vec<_>>>().ok_or(tr!("Campos calculados inválidos."))?,
        None => Vec::new(),
    };
    let nomes_reais = campos::campos(origem.clone(),
        |r,c| app.workbook.valor(folha_origem, r, c))
        .into_iter().map(|c| c.nome).collect::<Vec<_>>();
    let mut nomes = nomes_reais;
    nomes.extend(calculados.iter().map(|c| c.nome.clone()));
    let linhas = lista_indices(&meta["linhas"], &origem, calculados.len())
        .ok_or(tr!("Campos de linha inválidos."))?;
    let colunas = lista_indices(&meta["colunas"], &origem, calculados.len())
        .ok_or(tr!("Campos de coluna inválidos."))?;
    let valores = meta["valores"].as_array().ok_or(tr!("Medidas ausentes."))?
        .iter().map(|v| {
            Some(MedidaDinamica {
                coluna: indice(v.get("col").or_else(|| v.get("coluna"))?, &origem, calculados.len())?,
                op: agregacao(v.get("como").or_else(|| v.get("op"))?.as_str()?)?,
            })
        }).collect::<Option<Vec<_>>>()
        .ok_or(tr!("Medidas da dinâmica não reconhecidas."))?;
    let (layout,repetir_rotulos,subtotais,recolhidos,filtros) = avancado::opcoes_de_json(meta);
    let mut grupos: Vec<String> = meta["mapa"]["linhas"].as_array()
        .map(|itens| itens.iter().flat_map(|linha| {
            linha[2].as_array().into_iter().flatten()
                .filter_map(|marca| marca[1].as_str().map(str::to_owned))
        }).collect()).unwrap_or_default();
    grupos.sort();
    grupos.dedup();
    let filtros = filtros.into_iter().map(|(col, vals, excluir)| {
        let coluna = indice(&json!(col), &origem, calculados.len())?;
        Some(Filtro { coluna, valores: vals.into_iter().collect(), excluir })
    }).collect::<Option<Vec<_>>>().ok_or(tr!("Filtros fora do intervalo de origem."))?;
    let agrupar_data = meta["datas"].as_array()
        .and_then(|lista| lista.first())
        .and_then(|v| v["parte"].as_str())
        .and_then(AgruparData::do_str);
    let extras_campos = (linhas.clone(), colunas.clone(), valores.iter().map(|m| m.coluna).collect());
    Ok(Estado {
        origem: origem.clone(),
        origem_texto: endereco_area(&origem),
        recomendacoes: false,
        folha_origem, nomes, linhas, colunas, valores, agrupar_data,
        nova_aba: false, editando: Some(id), calculados,
        novo_nome: String::new(), nova_formula: String::new(),
        previa_calculado: None,
        layout, repetir_rotulos, subtotais, recolhidos, grupos, filtros,
        tema: meta["tema"].as_str().unwrap_or("padrao").to_owned(),
        campo_filtro: None, valores_filtro: Vec::new(), filtro_cortado: false,
        fase: Fase::Campos, avancado: false,
        ui: Default::default(),
        extras_campos, extras: serde_json::from_value(meta["extras"].clone()).unwrap_or_default(),
        atualizar_sozinha: meta["atualizarSozinha"].as_bool().unwrap_or(false),
        com_grafico: false,
        segmentacoes: meta["segmentacoes"].as_array().map(|lista| lista.iter().filter_map(|s| {
            let coluna = usize::try_from(s["col"].as_u64()?).ok()?;
            let sel = s["sel"].as_array()?.iter().filter_map(|v| v.as_str().map(str::to_owned)).collect();
            Some((coluna, sel))
        }).collect()).unwrap_or_default(),
        pagina_tambem: meta["paginaTambem"].as_array().map(|lista| lista.iter()
            .filter_map(|c| usize::try_from(c.as_u64()?).ok()).collect()).unwrap_or_default(),
    })
}

/// A dinâmica cujo resultado contém (`r`, `c`) da aba `aba`, se houver.
pub(super) fn localizar_na_celula(app: &App, aba: usize, r: u32, c: u32) -> Option<(usize, Json)> {
    let texto = app.workbook.aba(aba)?.sheet.dn.as_deref()?;
    let definicoes: Vec<Json> = serde_json::from_str(texto).ok()?;
    definicoes.into_iter().find(|d| d.get("areaResultado").and_then(retangulo_json)
        .is_some_and(|a| r >= a.r0 && r <= a.r1 && c >= a.c0 && c <= a.c1))
        .map(|d| (aba, d))
}

/// Permite abrir um resultado existente mesmo depois de salvar e reabrir a pasta.
pub(super) fn localizar_selecionada(app: &App) -> Result<(usize, usize, Json), String> {
    let destino = app.folha;
    let aba = app.workbook.aba(destino).ok_or(tr!("Aba da dinâmica não encontrada."))?;
    let texto = aba.sheet.dn.as_deref().ok_or(tr!("Esta aba não possui tabelas dinâmicas."))?;
    let definicoes: Vec<Json> = serde_json::from_str(texto)
        .map_err(|_| tr!("As definições de dinâmica estão inválidas."))?;
    let (indice, meta) = definicoes.iter().enumerate().find(|(_, d)| {
        d.get("areaResultado").and_then(retangulo_json).is_some_and(|r|
            app.linha >= r.r0 && app.linha <= r.r1 &&
            app.coluna >= r.c0 && app.coluna <= r.c1)
    }).ok_or(tr!("Selecione uma célula dentro de uma tabela dinâmica."))?;
    Ok((destino,indice,meta.clone()))
}

impl App {
    /// Clique no símbolo +/- da própria planilha: atualiza o mapa e os
    /// resultados sem exigir abrir a janela de configuração.
    pub(super) fn alternar_grupo_no_resultado(
        &mut self, linha: u32, coluna: u32, caminho: String,
    ) {
        self.confirmar();
        self.linha = linha;
        self.coluna = coluna;
        self.fim_selecao = (linha,coluna);
        let resultado = localizar_selecionada(self).and_then(|(destino,_,meta)| {
            let mut e = ler_estado(self,destino,&meta)?;
            if !e.grupos.contains(&caminho) {
                return Err(tr!("O grupo não existe mais. Atualize a dinâmica.").into());
            }
            if let Some(i) = e.recolhidos.iter().position(|g| g == &caminho) {
                e.recolhidos.remove(i);
            } else {
                e.recolhidos.push(caminho);
            }
            self.aplicar_estado_na_dinamica(destino,&e)
        });
        if let Err(erro) = resultado { self.status = erro; }
    }

    pub(super) fn abrir_edicao_dinamica(&mut self) {
        self.confirmar();
        match localizar_selecionada(self).and_then(|(destino,_,meta)| ler_estado(self,destino,&meta)) {
            Ok(estado) => {
                self.dinamica = Some(estado);
                self.contexto = None;
                self.menu_aberto = false;
                self.status = tr!("Editando a dinâmica existente. Aplicar mudanças recalcula o resultado.").into();
            }
            Err(erro) => self.status = erro,
        }
    }

    pub(super) fn atualizar_dinamica_existente(&mut self) {
        self.confirmar();
        match localizar_selecionada(self).and_then(|(destino,_,meta)| {
            let e = ler_estado(self,destino,&meta)?;
            self.aplicar_estado_na_dinamica(destino,&e)
        }) {
            Ok(()) => {}
            Err(erro) => self.status = erro,
        }
    }

    /// Dinâmicas com "Atualizar sozinha": se a aba de origem mudou desde a
    /// última conta, recalcula (pulso de 1 s). Nunca com o painel da
    /// dinâmica aberto nem no meio de uma edição, e sem tirar a pessoa da
    /// aba e da seleção em que está.
    pub(in crate::app) fn atualizar_dinamicas_sozinhas(&mut self) {
        if self.dinamica.is_some() || self.editor_formula.ativo { return; }
        let mut pendentes: Vec<(usize, Json, usize, String)> = Vec::new();
        for destino in 0..self.workbook.len() {
            let Some(txt) = self.workbook.aba(destino).and_then(|a| a.sheet.dn.as_deref()) else { continue };
            // A lista pode ser grande (o mapa de cada linha): só lê se precisa.
            if !txt.contains("\"atualizarSozinha\":true") { continue; }
            let Ok(lista) = serde_json::from_str::<Vec<Json>>(txt) else { continue };
            for meta in lista {
                if meta["atualizarSozinha"].as_bool() != Some(true) { continue; }
                let (Some(id), Some(fonte)) = (meta["id"].as_str(), meta["fonte"]["aba"].as_str()) else { continue };
                let Some(origem) = self.workbook.nomes().iter().position(|n| n == fonte) else { continue };
                let versao = self.workbook.aba(origem).map_or(0, |a| a.versao);
                match self.versoes_origem_dinamicas.get(id).copied() {
                    Some(v) if v == versao => {}
                    // Primeira vez: a conta gravada vale para a origem de agora.
                    None => { self.versoes_origem_dinamicas.insert(id.to_owned(), versao); }
                    Some(_) => pendentes.push((destino, meta.clone(), origem, id.to_owned())),
                }
            }
        }
        for (destino, meta, origem, id) in pendentes {
            let lugar = (self.folha, self.linha, self.coluna, self.linha_inicio, self.coluna_inicio,
                self.fim_selecao, self.endereco.clone(), self.status.clone());
            let feito = ler_estado(self, destino, &meta).and_then(|e| self.aplicar_estado_na_dinamica(destino, &e));
            (self.folha, self.linha, self.coluna, self.linha_inicio, self.coluna_inicio,
                self.fim_selecao, self.endereco, self.status) = lugar;
            if feito.is_ok() {
                feathersheet_core::diario::anotar(trf!("[dinâmica] {id} atualizada sozinha"));
            }
            // Com ou sem sucesso, só tenta de novo na próxima mudança da origem
            // (a escrita do resultado também pode ter mudado a versão dela).
            let versao = self.workbook.aba(origem).map_or(0, |a| a.versao);
            self.versoes_origem_dinamicas.insert(id, versao);
        }
    }

    pub(super) fn salvar_edicao_dinamica(&mut self, e: &Estado, id: &str) {
        let destino = (0..self.workbook.len()).find(|&i| self.workbook.aba(i)
            .and_then(|a| a.sheet.dn.as_deref())
            .and_then(|txt| serde_json::from_str::<Vec<Json>>(txt).ok())
            .is_some_and(|d| d.iter().any(|p| p["id"] == id)));
        let resultado = destino.ok_or(tr!("A dinâmica original não foi encontrada.").to_owned())
            .and_then(|destino| self.aplicar_estado_na_dinamica(destino,e));
        match resultado {
            Ok(()) => self.dinamica = None,
            Err(erro) => self.status = erro,
        }
    }

    /// Recalcula e grava a dinâmica. Origem grande vai para a conta de
    /// fundo ([`super::fundo`]) e grava quando terminar.
    pub(super) fn aplicar_estado_na_dinamica(&mut self, destino: usize, e: &Estado) -> Result<(), String> {
        if Self::dinamica_vai_para_fundo(e) {
            return self.iniciar_dinamica_fundo(destino, e);
        }
        self.aplicar_com_resultado(destino, e, None)
    }

    /// Grava a dinâmica; sem `pronto`, faz a conta aqui mesmo.
    pub(super) fn aplicar_com_resultado(&mut self, destino: usize, e: &Estado, pronto: Option<Resultado>) -> Result<(), String> {
        avancado::conferir_nomes_calculados(e)?;
        let origem_nome = self.workbook.aba(e.folha_origem)
            .ok_or(tr!("Aba de origem não encontrada."))?.sheet.name.clone();
        let aba = self.workbook.aba(destino).ok_or(tr!("Aba de destino não encontrada."))?;
        let raw = aba.sheet.dn.as_deref().ok_or(tr!("Definição da dinâmica não encontrada."))?;
        let mut lista: Vec<Json> = serde_json::from_str(raw)
            .map_err(|_| tr!("Metadados das dinâmicas inválidos; nada foi alterado."))?;
        let id = e.editando.as_deref()
            .ok_or(tr!("Falta o identificador da dinâmica para atualização."))?;
        let pos = lista.iter().position(|d| d["id"] == id)
            .ok_or(tr!("A dinâmica selecionada foi removida."))?;
        let antigo = retangulo_json(&lista[pos]["areaResultado"])
            .ok_or(tr!("Área do resultado anterior inválida."))?;
        let inicio = std::time::Instant::now();
        let resultado = match pronto {
            Some(r) => r,
            None => avancado::calcular(self,e)?,
        };
        let conta_ms = inicio.elapsed().as_millis();
        let altura = resultado.matriz.height();
        let largura = resultado.matriz.rows().iter().map(Vec::len).max().unwrap_or(0);
        if altura == 0 || largura == 0 { return Err(tr!("Nenhum resultado para a dinâmica.").into()); }
        // Filtros de relatório na grade, acima da tabela (como o Excel):
        // uma linha por campo e uma em branco; sem espaço, a tabela desce.
        let antiga_af = lista[pos].get("areaFiltros").and_then(retangulo_json);
        // Só os filtros de relatório (campo na área Filtros); o filtro de itens
        // de um campo de Linhas/Colunas fica no próprio campo.
        let paginas = e.filtros_de_relatorio();
        let n_filtros = paginas.len() as u32;
        let inicio_r = if n_filtros > 0 { antigo.r0.max(n_filtros + 1) } else { antigo.r0 };
        let nova_af = (n_filtros > 0).then(|| Rect::new(inicio_r - n_filtros - 1, antigo.c0, inicio_r - 2, antigo.c0 + 1));
        let fim_linha = (inicio_r as usize).checked_add(altura)
            .ok_or(tr!("Linhas de destino fora dos limites."))?;
        let fim_coluna = (antigo.c0 as usize).checked_add(largura)
            .ok_or(tr!("Colunas de destino fora dos limites."))?;
        if fim_linha > MAX_ROWS as usize || fim_coluna > MAX_COLS as usize {
            return Err(tr!("O resultado excederia os limites da planilha.").into());
        }
        let nova = Rect::new(inicio_r, antigo.c0, fim_linha as u32 - 1, fim_coluna as u32 - 1);
        // Não permitir que o crescimento toque em outra tabela dinâmica.
        if lista.iter().enumerate().any(|(i, d)| i != pos &&
            d.get("areaResultado").and_then(retangulo_json).is_some_and(|r|
                r.r0 <= nova.r1 && nova.r0 <= r.r1 &&
                r.c0 <= nova.c1 && nova.c0 <= r.c1)) {
            return Err(tr!("O resultado cruzaria outra tabela dinâmica existente.").into());
        }
        let aba = &self.workbook.aba(destino).expect("aba validada").sheet;
        for (i, row) in resultado.matriz.rows().iter().enumerate() {
            for j in 0..row.len() {
                let r = nova.r0 + i as u32;
                let c = nova.c0 + j as u32;
                let dentro_antigo = r >= antigo.r0 && r <= antigo.r1
                    && c >= antigo.c0 && c <= antigo.c1;
                if !dentro_antigo && !aba.get_raw(r,c).is_empty() {
                    return Err(tr!("A expansão da dinâmica atingiria dados existentes; não houve alteração.").into());
                }
            }
        }
        if let Some(af) = &nova_af {
            for r in af.r0..=af.r1 {
                for c in af.c0..=af.c1 {
                    let livre = antigo.contains(r, c) || antiga_af.as_ref().is_some_and(|a| a.contains(r, c))
                        || aba.get_raw(r, c).is_empty();
                    if !livre {
                        return Err(tr!("Não há espaço livre acima da dinâmica para os filtros; não houve alteração.").into());
                    }
                }
            }
        }
        let celulas_filtro: Vec<(String, String)> = paginas.iter().map(|f| {
            let item = if f.excluir && f.valores.is_empty() { tr!("(Tudo)").to_string() }
                else if !f.excluir && f.valores.len() == 1 { f.valores.iter().next().cloned().unwrap_or_default() }
                else { tr!("(Vários itens)").to_string() };
            (e.nome_da_coluna(f.coluna), item)
        }).collect();
        let m = &mut lista[pos];
        m["fonte"] = json!({"aba":origem_nome,"area":{
            "r0":e.origem.r0,"r1":e.origem.r1,"c0":e.origem.c0,"c1":e.origem.c1
        }});
        avancado::gravar_campos(m,e);
        m["areaResultado"] = json!({"r0":nova.r0,"c0":nova.c0,"r1":nova.r1,"c1":nova.c1});
        m["areaFiltros"] = nova_af.as_ref().map_or(Json::Null, |a| json!({"r0":a.r0,"c0":a.c0,"r1":a.r1,"c1":a.c1}));
        // Funil cheio na grade: o filtro de relatório esconde algo.
        m["filtrosPagina"] = json!(paginas.iter().map(|f| !(f.excluir && f.valores.is_empty())).collect::<Vec<_>>());
        avancado::gravar_opcoes(m,e,avancado::mapa_json(&resultado.linhas, resultado.colunas_de_rotulo));
        let gravar = serde_json::to_string(&lista)
            .map_err(|_| tr!("Falha ao serializar as opções da dinâmica."))?;
        let extras = avancado::extras_vigentes(e);
        self.workbook.editar(destino, |sheet| {
            sheet.abrir_transacao();
            let mut escritas = 0usize;
            for r in antigo.r0..=antigo.r1 {
                for c in antigo.c0..=antigo.c1 {
                    // Limpar apenas células ocupadas evita criar snapshots
                    // desnecessários para espaços em branco do resultado. O
                    // que fica dentro da área nova é sobrescrito logo abaixo.
                    if !nova.contains(r, c) && !sheet.get_raw(r,c).is_empty() {
                        sheet.set_cell(r,c,"");
                        escritas += 1;
                        if escritas % 16_384 == 0 { sheet.compactar_lote_divisao(); }
                    }
                }
            }
            // Caminho rápido do Texto para Colunas: analisa o texto uma vez e
            // grava direto, com o mesmo passo de Ctrl+Z (era 1 milhão de
            // `set_cell` completos numa dinâmica de 1 milhão de linhas).
            for (i,row) in resultado.matriz.rows().iter().enumerate() {
                let r = nova.r0 + i as u32;
                for j in 0..largura {
                    let c = nova.c0 + j as u32;
                    match row.get(j) {
                        Some(v) => sheet.set_cell_divisao(r, c, &texto_celula(v)),
                        None if !sheet.get_raw(r, c).is_empty() => sheet.set_cell(r, c, ""),
                        None => continue,
                    }
                    escritas += 1;
                    if escritas % 16_384 == 0 { sheet.compactar_lote_divisao(); }
                }
            }
            // Filtros acima da tabela: limpa os de antes e escreve os de agora.
            if let Some(af) = &antiga_af {
                for r in af.r0..=af.r1 {
                    for c in af.c0..=af.c1 {
                        if !nova.contains(r, c) && !sheet.get_raw(r, c).is_empty() { sheet.set_cell(r, c, ""); }
                    }
                }
            }
            if let Some(af) = &nova_af {
                for (i, (campo, item)) in celulas_filtro.iter().enumerate() {
                    sheet.set_cell(af.r0 + i as u32, af.c0, &texto_celula(&Value::Text(campo.clone())));
                    sheet.set_cell(af.r0 + i as u32, af.c0 + 1, &texto_celula(&Value::Text(item.clone())));
                }
            }
            sheet.compactar_lote_divisao();
            super::config_campo::formatar_valores(sheet, &nova, &resultado, &extras, e.valores.len());
            sheet.alterar_dinamicas(Some(gravar), &nova);
            sheet.fechar_transacao();
        });
        avancado::guardar_caminhos(self, e, &resultado);
        let total_ms = inicio.elapsed().as_millis();
        let linhas_origem = e.origem.r1 - e.origem.r0;
        feathersheet_core::diario::anotar(trf!(
            "[dinâmica] conta {conta_ms} ms e gravação {} ms ({linhas_origem} linhas da origem, resultado {altura} × {largura})",
            total_ms - conta_ms));
        self.alterado = true;
        let meta_novo = lista[pos].clone();
        self.seguir_graficos_dinamicos(destino, &meta_novo);
        // A aba, a seleção e a rolagem ficam onde a pessoa estava (#106): antes
        // cada mudança no painel selecionava a tabela inteira e rolava até ela.
        // Mas a célula ativa pode ter mudado de conteúdo: a barra fx acompanha.
        if self.folha == destino { self.recarregar_barra_fx(); }
        self.status = trf!("Tabela dinâmica atualizada: {} linhas × {} colunas.",altura,largura);
        Ok(())
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn clique_no_grupo_recolhe_e_expande_sem_modificar_origem() {
        let mut app=App::default();
        app.workbook.editar(0,|s|{
            s.set_cell(0,0,"Grupo");s.set_cell(0,1,"Item");s.set_cell(0,2,"Valor");
            s.set_cell(1,0,"a");s.set_cell(1,1,"b");s.set_cell(1,2,"3");
            s.set_cell(2,0,"a");s.set_cell(2,1,"c");s.set_cell(2,2,"5");
            s.set_cell(3,0,"d");s.set_cell(3,1,"e");s.set_cell(3,2,"1");
        });
        app.fim_selecao=(3,2);
        app.evento_dinamica(Evento::Abrir);
        crate::app::dinamica_ui::campos_de_antes(&mut app);
        app.evento_dinamica(Evento::Linha(1));
        app.evento_dinamica(Evento::Layout);
        app.evento_dinamica(Evento::Criar);
        assert_eq!(app.workbook.valor(1,1,0),Value::Text("a".into()));
        app.alternar_grupo_no_resultado(1,0,"a".into());
        let meta = app.workbook.aba(1).unwrap().sheet.dn.as_ref().unwrap().clone();
        assert!(meta.contains("recolhidos"));
        let meta_json: serde_json::Value = serde_json::from_str(&meta).unwrap();
        assert_eq!(meta_json[0]["recolhidos"],json!(["a"]));
        assert_eq!(app.workbook.valor(0,2,2),Value::Number(5.));
        app.alternar_grupo_no_resultado(1,0,"a".into());
        assert_eq!(app.workbook.valor(1,2,0),Value::Text("b".into()));
    }

    #[test]
    fn operacoes_e_campos_legados_do_typescript_sao_lidos() {
        assert_eq!(agregacao("conta"),Some(AgregacaoOp::Contagem));
        assert_eq!(agregacao("contar"),Some(AgregacaoOp::Contagem));
        assert_eq!(agregacao("desvioPadrao"),Some(AgregacaoOp::DesvioPadrao));
        assert_eq!(agregacao("minimo"),Some(AgregacaoOp::Min));
        assert_eq!(agregacao("max"),Some(AgregacaoOp::Max));

        let mut app=App::default();
        app.workbook.editar(0,|s|{
            s.set_cell(0,0,"Produto"); s.set_cell(0,1,"Valor");
            s.set_cell(1,0,"X"); s.set_cell(1,1,"10");
            s.set_cell(2,0,"X"); s.set_cell(2,1,"20");
        });
        let meta = json!({
            "id":"pivot-legado",
            "fonte":{"aba":"Planilha1","area":{"r0":0,"c0":0,"r1":2,"c1":1}},
            "linhas":[0], "colunas":[],
            "valores":[{"coluna":1,"op":"conta"}],
            "areaResultado":{"r0":0,"c0":2,"r1":2,"c1":3}
        });
        let e = ler_estado(&app,0,&meta).unwrap();
        assert_eq!(e.valores[0].coluna,1);
        assert_eq!(e.valores[0].op,AgregacaoOp::Contagem);
    }

    /// A definição que o leitor de `.xlsx` grava (colunas absolutas,
    /// `como`, `extras` com a ordem do Excel, `areaResultado` da `location`)
    /// é a mesma que o painel atualiza.
    #[test]
    fn dinamica_lida_do_xlsx_atualiza_pelo_painel_com_os_extras() {
        let mut app=App::default();
        app.workbook.criar_aba("Dados");
        app.workbook.editar(1,|s|{
            s.set_cell(0,1,"Tipo");s.set_cell(0,2,"Valor");
            s.set_cell(1,1,"Serviço");s.set_cell(1,2,"100");
            s.set_cell(2,1,"Produto");s.set_cell(2,2,"30");
            s.set_cell(3,1,"Serviço");s.set_cell(3,2,"50");
        });
        let meta = json!([{
            "id":"din_0_0",
            "fonte":{"aba":"Dados","area":{"r0":0,"c0":1,"r1":3,"c1":2}},
            "destino":{"row":1,"col":0},
            "linhas":[1],"colunas":[],
            "valores":[{"col":2,"como":"soma","nome":"Valor"}],
            "extras":{"valores":[{"tipo":"perc_total_geral"}],
                "linhas":[{"agrupamento":null,"ordenacao":{"tipo":"decrescente"}}],"colunas":[],
                "totais_gerais_linhas":true,"totais_gerais_colunas":true},
            "layout":"tabular","repetirRotulos":false,"subtotais":false,
            "areaResultado":{"r0":1,"c0":0,"r1":4,"c1":1}
        }]);
        app.workbook.editar(0,|s| s.dn = Some(meta.to_string()));
        app.folha=0; app.linha=2; app.coluna=0; app.fim_selecao=(2,0);
        app.atualizar_dinamica_existente();
        assert_eq!(app.workbook.valor(0,2,0),Value::Text("Serviço".into()));
        assert_eq!(app.workbook.valor(0,3,0),Value::Text("Produto".into()));
        let parte = |v: Value| match v { Value::Number(n) => (n*1000.).round()/1000., outro => panic!("{outro:?}") };
        assert_eq!(parte(app.workbook.valor(0,2,1)),0.833);
        assert_eq!(parte(app.workbook.valor(0,4,1)),1.0);
        let gravada: serde_json::Value = serde_json::from_str(
            app.workbook.aba(0).unwrap().sheet.dn.as_deref().unwrap()).unwrap();
        assert_eq!(gravada[0]["extras"]["valores"][0]["tipo"],"perc_total_geral");
    }

    #[test]
    fn indices_negativos_do_ts_apontam_para_campos_calculados() {
        let origem=Rect::new(0,2,20,4);
        assert_eq!(indice(&json!(-1),&origem,2),Some(5));
        assert_eq!(indice(&json!(-2),&origem,2),Some(6));
        assert_eq!(indice(&json!(-3),&origem,2),None);
    }
    #[test]
    fn filtros_e_calculados_do_painel_rust_sao_aplicados_e_persistidos() {
        let mut app=App::default();
        app.workbook.editar(0,|sheet|{
            sheet.set_cell(0,0,"Região");sheet.set_cell(0,1,"Preço");sheet.set_cell(0,2,"Quantidade");
            sheet.set_cell(1,0,"Sul");sheet.set_cell(1,1,"10");sheet.set_cell(1,2,"2");
            sheet.set_cell(2,0,"Norte");sheet.set_cell(2,1,"20");sheet.set_cell(2,2,"3");
            sheet.set_cell(3,0,"Sul");sheet.set_cell(3,1,"7");sheet.set_cell(3,2,"4");
        });
        app.fim_selecao=(3,2);
        app.evento_dinamica(Evento::Abrir);
        crate::app::dinamica_ui::campos_de_antes(&mut app);
        app.evento_dinamica(Evento::NomeCalculado("Receita".into()));
        app.evento_dinamica(Evento::FormulaCalculado("[Preço] * Quantidade".into()));
        app.evento_dinamica(Evento::AdicionarCalculado);
        app.evento_dinamica(Evento::Valor(2)); // Remover soma de Quantidade.
        app.evento_dinamica(Evento::Criar);
        assert_eq!(app.workbook.len(),2);
        assert_eq!(app.workbook.valor(1,1,1),Value::Number(60.));
        assert_eq!(app.workbook.valor(1,2,1),Value::Number(48.));
        let meta = app.workbook.aba(1).unwrap().sheet.dn.as_ref().unwrap();
        assert!(meta.contains("Receita"));
        assert!(meta.contains("calculados"));
        app.evento_dinamica(Evento::Editar);
        let e = app.dinamica.as_ref().unwrap();
        assert_eq!(e.calculados.len(),1);
        assert_eq!(e.valores.len(),1);
        app.evento_dinamica(Evento::FiltroCampo(0));
        app.evento_dinamica(Evento::FiltroValor("Norte".into()));
        app.evento_dinamica(Evento::Criar);
        assert_eq!(app.workbook.valor(1,1,0),Value::Text("Sul".into()));
        assert_eq!(app.workbook.valor(1,1,1),Value::Number(48.));
        assert_eq!(app.workbook.valor(1,2,1),Value::Number(48.));
        let meta = app.workbook.aba(1).unwrap().sheet.dn.as_ref().unwrap();
        assert!(meta.contains("Norte"));
    }

    #[test]
    fn editar_dinamica_persiste_layout_e_recalcula() {
        let mut app=App::default();
        app.workbook.editar(0,|sheet|{
            sheet.set_cell(0,0,"Região");sheet.set_cell(0,1,"Valor");
            sheet.set_cell(1,0,"Sul");sheet.set_cell(1,1,"10");
            sheet.set_cell(2,0,"Sul");sheet.set_cell(2,1,"20");
        });
        app.fim_selecao=(2,1);
        app.evento_dinamica(Evento::Abrir);
        crate::app::dinamica_ui::campos_de_antes(&mut app);
        app.evento_dinamica(Evento::Criar);
        app.evento_dinamica(Evento::Editar);
        assert!(app.dinamica.as_ref().unwrap().editando.is_some());
        app.evento_dinamica(Evento::Layout);
        app.evento_dinamica(Evento::Criar);
        assert_eq!(app.workbook.aba(1).unwrap().sheet.dn.as_ref()
            .unwrap().contains("compacto"),true);
    }

    #[test]
    fn dinamica_com_atualizar_sozinha_acompanha_a_origem() {
        let mut app = App::default();
        app.workbook.editar(0, |s| {
            s.set_cell(0, 0, "Região"); s.set_cell(0, 1, "Valor");
            s.set_cell(1, 0, "Norte"); s.set_cell(1, 1, "10");
            s.set_cell(2, 0, "Sul"); s.set_cell(2, 1, "30");
        });
        app.fim_selecao = (2, 1);
        app.evento_dinamica(Evento::Abrir);
        crate::app::dinamica_ui::campos_de_antes(&mut app);
        app.evento_dinamica(Evento::AtualizarSozinha(true));
        app.evento_dinamica(Evento::Criar);
        app.dinamica = None;
        let destino = app.folha;
        let norte = |app: &App| {
            let s = &app.workbook.aba(destino).unwrap().sheet;
            (0..6).find(|&r| s.get_raw(r, 0) == "Norte").map(|r| s.get_raw(r, 1))
        };
        assert_eq!(norte(&app).as_deref(), Some("10"));
        app.atualizar_dinamicas_sozinhas(); // primeira vez só anota a versão
        app.folha = 0;
        app.workbook.escrever_celula(0, 1, 1, "15".into());
        app.atualizar_dinamicas_sozinhas();
        assert_eq!(norte(&app).as_deref(), Some("15"));
        assert_eq!(app.folha, 0, "não tira a pessoa da aba em que está");
    }
}

