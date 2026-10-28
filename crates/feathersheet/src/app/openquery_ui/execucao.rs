//! Execução de consultas gravadas. Preserva o vínculo ao destino e relê a
//! origem na atualização. Só modifica o retângulo que a consulta criou.
use super::*;

impl App {
    pub(super) fn abrir_consulta_salva(&mut self, id: &str) -> bool {
        let resultado = (|| -> Result<Estado, String> {
            let salva = registro::buscar(&self.workbook, id)?;
            let (origem_aba, origem, fonte) =
                if let Some((nome, area)) = registro::origem(&salva.dados) {
                    if nome.starts_with("Arquivo: ") || nome.starts_with("Web: ") || nome == "Entrada manual" {
                        // Arquivo importado: reabre o snapshot salvo. Atualizar
                        // arquivo externo exige reimportação explícita, nunca
                        // leitura silenciosa de caminho arbitrário no disco.
                        (nome, area, registro::fonte_snapshot(&salva.dados)?)
                    } else {
                        let indice = self.workbook.indice_por_nome(&nome).ok_or_else(|| {
                            trf!("A origem '{nome}' foi removida; a consulta não pode ser atualizada")
                        })?;
                        let tabela = ler_tabela(&self.workbook, indice, area.clone())?;
                        (nome, area, tabela)
                    }
                } else {
                    // Consultas legadas TS podem não possuir a referência da
                    // origem. É possível editá-las a partir do snapshot,
                    // mas não sobrescrever o resultado sem a área gerada.
                    let fonte = registro::fonte_snapshot(&salva.dados)?;
                    let area = Rect::new(
                        0, 0, fonte.linhas.len() as u32,
                        fonte.cabecalho.len().saturating_sub(1) as u32,
                    );
                    (tr!("Consulta salva").into(), area, fonte)
                };
            let mut estado = Estado::novo(fonte, origem_aba, origem);
            estado.nome = salva.dados["nome"].as_str().unwrap_or(tr!("Consulta")).into();
            estado.edicao = Some(id.to_owned());
            estado.destino_modo = match salva.dados["destino"]["modo"].as_str() {
                Some("apenas_conexao") => DestinoModo::ApenasConexao,
                Some("aba_atual") => DestinoModo::AbaAtual,
                _ => DestinoModo::NovaAba,
            };
            let texto = serde_json::to_string(&salva.dados["passos"])
                .map_err(|e| e.to_string())?;
            estado.passos = json_passos::ler(&texto)?;
            for nome in registro::fontes_extra(&salva.dados) {
                let (indice, area) = if let Some(tabela) = nome.strip_prefix("Tabela: ") {
                    tabelas_existentes(&self.workbook).into_iter()
                        .find(|(_, n, _)| n == tabela)
                        .map(|(i, _, a)| (i, a))
                        .ok_or_else(|| trf!("A tabela adicional '{tabela}' não existe mais"))?
                } else {
                    // Compatibilidade com consultas antigas que mesclavam abas.
                    let indice = self.workbook.indice_por_nome(&nome).ok_or_else(|| {
                        trf!("A fonte adicional '{nome}' não existe mais")
                    })?;
                    let area = self.workbook.aba(indice).ok_or(tr!("Aba indisponível"))?.sheet.used_rect();
                    (indice, area)
                };
                estado.fontes.insert(nome, ler_tabela(&self.workbook, indice, area)?);
            }
            estado.registrar_passos();
            if let Some(url) = estado.origem_aba.strip_prefix("Web: ") {
                estado.url_web = url.to_owned();
            }
            if !estado.erro.is_empty() { return Err(estado.erro); }
            estado.salvas = registro::listar(&self.workbook)?.into_iter()
                .filter_map(|salva| Some((
                    salva.dados["id"].as_str()?.to_string(),
                    salva.dados["nome"].as_str()?.to_string(),
                ))).collect();
            Ok(estado)
        })();
        match resultado {
            Ok(estado) => {
                self.openquery = Some(estado);
                self.contexto = None;
                self.menu_aberto = false;
                self.status = tr!("OpenQuery: consulta carregada para edição").into();
                true
            }
            Err(erro) => {
                if let Some(editor) = self.openquery.as_mut() { editor.erro = erro.clone(); }
                self.status = erro;
                false
            }
        }
    }

    /// Apenas conexão: guarda receita e snapshot sem criar/modificar
    /// células da grade. O registro mora numa aba existente para que a
    /// serialização FSHZ continue no contrato Sheet.oq do TypeScript.
    pub(super) fn salvar_conexao_openquery(&mut self) {
        let preparado = (|| -> Result<(usize, serde_json::Value, String), String> {
            let editor = self.openquery.as_mut().ok_or(tr!("Editor de consulta não está aberto"))?;
            editor.sincronizar()?;
            editor.recalcular();
            if !editor.erro.is_empty() { return Err(editor.erro.clone()); }
            let id = if let Some(id) = &editor.edicao {
                let anterior = registro::buscar(&self.workbook, id)?;
                if registro::area_saida(&anterior.dados).is_some() {
                    return Err(tr!("Para converter uma consulta materializada em conexão,                          desvincule primeiro sua tabela de resultado").into());
                }
                id.to_owned()
            } else {
                registro::proximo_id(&self.workbook)?
            };
            let aba = if let Some(id) = &editor.edicao {
                let anterior = registro::buscar(&self.workbook, id)?;
                registro::destino(&anterior.dados)
                    .and_then(|nome| self.workbook.indice_por_nome(nome))
                    .unwrap_or(self.folha)
            } else { self.folha };
            let nome_aba = self.workbook.aba(aba)
                .ok_or(tr!("Aba escolhida para armazenar a conexão não existe"))?
                .sheet.name.clone();
            let nome = if editor.nome.trim().is_empty() { tr!("Consulta") } else {
                editor.nome.trim()
            };
            let adicionais: Vec<String> = editor.fontes.keys().cloned().collect();
            let area = Rect::new(0, 0, 0, 0);
            let mut reg = registro::montar(
                &id, nome, &editor.fonte, &editor.passos, &editor.origem_aba,
                &editor.origem, &nome_aba, &area, &adicionais,
            )?;
            reg["destino"]["modo"] = serde_json::json!("apenas_conexao");
            reg["_feather"]["areaSaida"] = serde_json::Value::Null;
            reg["status"]["linhasAfetadas"] = serde_json::json!(0);
            // Também valida metadados de outras abas antes de qualquer escrita.
            registro::preparar_gravacao(&self.workbook, aba, &reg)?;
            Ok((aba, reg, id))
        })();
        match preparado {
            Ok((aba, reg, id)) => {
                match registro::preparar_gravacao(&self.workbook, aba, &reg) {
                    Ok(plano) => {
                        registro::aplicar_gravacao(&mut self.workbook, plano);
                        self.alterado = true;
                        self.openquery = None;
                        self.status = trf!("OpenQuery '{id}': conexão salva sem escrever células");
                    }
                    Err(erro) => {
                        if let Some(e) = &mut self.openquery { e.erro = erro.clone(); }
                        self.status = erro;
                    }
                }
            }
            Err(erro) => {
                if let Some(e) = &mut self.openquery { e.erro = erro.clone(); }
                self.status = erro;
            }
        }
    }

    /// Salva e carrega: retorna erro no editor sem descartar o rascunho.
    pub(super) fn concluir_openquery(&mut self) {
        let preparado = (|| -> Result<
            (String, String, usize, Option<Rect>, DestinoModo, serde_json::Value, vinculo::Plano), String
        > {
            let editor = self.openquery.as_mut().ok_or(tr!("Editor não está aberto"))?;
            editor.sincronizar()?;
            editor.recalcular();
            if !editor.erro.is_empty() { return Err(editor.erro.clone()); }
            let tabela = &editor.resultado;
            if tabela.cabecalho.is_empty() {
                return Err(tr!("A consulta não tem colunas para carregar").into());
            }
            let linhas = tabela.linhas.len() as u64 + 1;
            let colunas = tabela.linhas.iter().map(Vec::len)
                .max().unwrap_or(0).max(tabela.cabecalho.len()) as u64;
            if linhas > MAX_ROWS as u64 || colunas > MAX_COLS as u64 {
                return Err(tr!("Resultado acima do orçamento OpenQuery").into());
            }
            let id = match &editor.edicao {
                Some(id) => id.clone(),
                None => registro::proximo_id(&self.workbook)?,
            };
            let nome_consulta = editor.nome.trim();
            let nome_consulta = if nome_consulta.is_empty() { tr!("Consulta") } else { nome_consulta };
            let (aba, nome_aba, anterior) = if let Some(id) = &editor.edicao {
                let salva = registro::buscar(&self.workbook, id)?;
                // Legacy TS não registra a área de saída. É inseguro
                // sobrescrevê-la, sobretudo com outras tabelas na aba.
                let antigo = registro::area_saida(&salva.dados).ok_or(
                    tr!("Esta consulta antiga não tem área de saída vinculada. \
                     Não sobrescrevi a aba; crie a consulta de novo.")
                )?;
                let nome = registro::destino(&salva.dados).ok_or(tr!("Destino salvo inválido"))?;
                let indice = self.workbook.indice_por_nome(nome)
                    .ok_or(tr!("A aba de resultado vinculada não existe mais"))?;
                if antigo.r0 != 0 || antigo.c0 != 0 {
                    return Err(tr!("Somente atualizações ancoradas em A1 estão disponíveis").into());
                }
                // Crescimento não deve apagar valores manuais fora da antiga
                // área gerada. Validar antes de escrever qualquer célula.
                let planilha = &self.workbook.aba(indice)
                    .ok_or(tr!("Aba de resultado indisponível"))?.sheet;
                for linha in 0..linhas as u32 {
                    for coluna in 0..colunas as u32 {
                        if (linha > antigo.r1 || coluna > antigo.c1)
                            && planilha.snapshot(linha, coluna).is_some() {
                            return Err(trf!(
                                "O resultado atingiria {} com dados fora da área anterior; \
                                 libere o destino antes de atualizar.",
                                cell_name(linha, coluna),
                            ));
                        }
                    }
                }
                (indice, nome.to_string(), Some(antigo))
            } else if editor.destino_modo == DestinoModo::AbaAtual {
                // Carga nova em aba existente: não substitui células com
                // valores nem células apenas formatadas sem confirmação.
                let aba = self.folha;
                let sheet = &self.workbook.aba(aba).ok_or(
                    tr!("Aba atual de destino não está disponível")
                )?.sheet;
                for r in 0..linhas as u32 {
                    for c in 0..colunas as u32 {
                        if sheet.snapshot(r,c).is_some() {
                            return Err(trf!(
                                "A aba atual já contém dados em {}.                                  Escolha uma aba vazia ou 'Nova aba' para preservar seu trabalho.",
                                cell_name(r,c),
                            ));
                        }
                    }
                }
                (aba, sheet.name.clone(), None)
            } else {
                // Sem alterar o workbook ainda: a criação ocorre depois da
                // validação de todos os metadados e da origem.
                let nomes = self.workbook.nomes();
                let base: String = nome_consulta.chars().map(|c| {
                    if matches!(c, '/' | '\\' | '!' | '[' | ']') { '_' } else { c }
                }).collect();
                let base = if base.trim().is_empty() { tr!("Consulta") } else { &base };
                let mut novo = base.to_owned();
                let mut numero = 2;
                while nomes.iter().any(|n| n.eq_ignore_ascii_case(&novo)) {
                    novo = format!("{base}_{numero}");
                    numero += 1;
                }
                (self.workbook.len(), novo, None)
            };
            let area_saida = feathersheet_core::consulta::sheet_query::area_saida(
                0, 0, tabela, true, u64::MAX,
            )?;
            let adicionais: Vec<String> = editor.fontes.keys().cloned().collect();
            let mut reg = registro::montar(
                &id, nome_consulta, &editor.fonte, &editor.passos,
                &editor.origem_aba, &editor.origem, &nome_aba,
                &area_saida, &adicionais
            )?;
            if editor.destino_modo == DestinoModo::AbaAtual {
                reg["destino"]["modo"] = serde_json::json!("aba_atual");
            }
            // Garantir que a área de saída não cruze outra tabela, antes
            // de alterar qualquer dado no Workbook.
            let plano_tabela = vinculo::preparar(
                &self.workbook,
                if anterior.is_some() || editor.destino_modo == DestinoModo::AbaAtual {
                    Some(aba)
                } else { None },
                &id, nome_consulta, area_saida,
            )?;
            registro::listar(&self.workbook)?;
            if anterior.is_some() {
                registro::preparar_gravacao(&self.workbook, aba, &reg)?;
            }
            Ok((id, nome_aba, aba, anterior, editor.destino_modo, reg, plano_tabela))
        })();
        let (id, nome_aba, destino, anterior, modo_destino, registro, plano_tabela) = match preparado {
            Ok(dados) => dados,
            Err(erro) => {
                if let Some(e) = &mut self.openquery { e.erro = erro.clone(); }
                self.status = erro;
                return;
            }
        };
        let editor = self.openquery.take().expect("Editor validado");
        let linhas_geradas = editor.resultado.linhas.len();
        let destino = if anterior.is_some() || modo_destino == DestinoModo::AbaAtual {
            destino
        } else {
            self.workbook.criar_aba(nome_aba.clone())
        };
        let mut plano = match registro::preparar_gravacao(&self.workbook, destino, &registro) {
            Ok(plano) => plano,
            Err(erro) => {
                self.openquery = Some(editor);
                self.status = erro.clone();
                if let Some(e) = &mut self.openquery { e.erro = erro; }
                return;
            }
        };
        // A ponte sheet-query pertence ao motor, não à interface:
        // a escrita é tipada e apaga apenas a área da consulta anterior.
        // Em falha, o editor permanece aberto e o vínculo não é atualizado.
        let mut falha_escrita = None;
        self.workbook.editar(destino, |sheet| {
            match feathersheet_core::consulta::sheet_query::escrever_tabela(
                sheet, 0, 0, &editor.resultado, true,
                anterior.clone(), u64::MAX,
            ) {
                Ok(_) => {
                    // Data, hora, %… da fonte voltam com o formato (senão a aba mostra 46322).
                    let linhas = editor.resultado.linhas.len() as u32;
                    for (c, nome) in editor.resultado.cabecalho.iter().enumerate() {
                        let Some(estilo) = editor.formatos.get(nome) else { continue };
                        let mut patch = feathersheet_core::celulas::estilo_parcial::de_style(estilo);
                        patch.retain(|k, _| matches!(k.as_str(), "format" | "code" | "decimals"));
                        if linhas > 0 && !patch.is_empty() {
                            sheet.formatar_faixa(&Rect::new(1, c as u32, linhas, c as u32), &patch);
                        }
                    }
                    sheet.clear_history();
                }
                Err(erro) => falha_escrita = Some(erro),
            }
        });
        if let Some(erro) = falha_escrita {
            self.status = erro.clone();
            self.openquery = Some(editor);
            if let Some(e) = &mut self.openquery { e.erro = erro; }
            return;
        }
        registro::aplicar_gravacao(&mut self.workbook, std::mem::take(&mut plano));
        vinculo::instalar(&mut self.workbook, destino, plano_tabela);
        self.folha = destino;
        self.linha = 0;
        self.coluna = 0;
        self.fim_selecao = (0, 0);
        self.nome_aba = nome_aba.clone();
        self.endereco = "A1".into();
        self.alterado = true;
        self.status = trf!("OpenQuery '{id}': {linhas_geradas} linhas em {nome_aba}; consulta salva no FSHZ");
    }

    pub(super) fn atualizar_consulta_salva(&mut self, id: &str) {
        if self.abrir_consulta_salva(id) {
            self.concluir_openquery();
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn apenas_conexao_grava_receita_sem_criar_aba_nem_escrever_celulas() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 0, 0, "MWh".into());
        app.workbook.escrever_celula(0, 1, 0, "42".into());
        abrir_aba_para_teste(&mut app);
        app.evento_openquery(Evento::Destino(DestinoModo::ApenasConexao));
        app.evento_openquery(Evento::Carregar);
        assert_eq!(app.workbook.len(),1);
        assert_eq!(app.workbook.valor(0,1,0),Value::Number(42.));
        let salva = registro::listar(&app.workbook).unwrap();
        assert_eq!(salva.len(),1);
        assert_eq!(salva[0].dados["destino"]["modo"],"apenas_conexao");
        assert!(salva[0].dados["_feather"]["areaSaida"].is_null());
        let id = salva[0].dados["id"].as_str().unwrap().to_string();
        assert!(app.abrir_consulta_salva(&id));
        assert_eq!(app.openquery.as_ref().unwrap().destino_modo,DestinoModo::ApenasConexao);
    }

    #[test]
    fn apenas_conexao_sobrevive_roundtrip_fshz_e_renovacao() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 0, 0, "ID".into());
        app.workbook.escrever_celula(0, 1, 0, "1".into());
        abrir_aba_para_teste(&mut app);
        app.evento_openquery(Evento::Destino(DestinoModo::ApenasConexao));
        app.salvar_conexao_openquery();
        let bytes=feathersheet_io::da_workbook::pasta_gravar_bytes(&app.workbook,0).unwrap();
        let wb=feathersheet_io::para_workbook::para_workbook(
            &feathersheet_io::fshz_read::ler_fshz(&bytes).unwrap());
        let id=registro::listar(&wb).unwrap()[0].dados["id"]
            .as_str().unwrap().to_owned();
        let mut carregado=App::default();
        carregado.workbook=wb;
        carregado.workbook.escrever_celula(0,1,0,"2".into());
        carregado.atualizar_consulta_salva(&id);
        assert_eq!(carregado.workbook.len(),1);
        assert_eq!(carregado.workbook.valor(0,1,0),Value::Number(2.));
        assert_eq!(registro::buscar(&carregado.workbook,&id)
            .unwrap().dados["destino"]["modo"],"apenas_conexao");
    }

    #[test]
    fn aba_atual_rejeita_colisao_sem_alterar_entrada() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 0, 0, "ID".into());
        app.workbook.escrever_celula(0, 1, 0, "9".into());
        abrir_aba_para_teste(&mut app);
        app.evento_openquery(Evento::Destino(DestinoModo::AbaAtual));
        app.concluir_openquery();
        assert_eq!(app.workbook.len(),1);
        assert!(app.status.contains("já contém dados"));
        assert_eq!(app.workbook.valor(0,1,0),Value::Number(9.));
        assert!(registro::listar(&app.workbook).unwrap().is_empty());
    }
    #[test]
    fn aba_atual_vazia_recebe_resultado_e_vinculo() {
        let mut app = App::default();
        app.workbook.escrever_celula(0,0,0,"Código".into());
        app.workbook.escrever_celula(0,1,0,"42".into());
        abrir_aba_para_teste(&mut app);
        let nova=app.workbook.criar_aba("Destino");
        app.folha=nova;
        app.evento_openquery(Evento::Destino(DestinoModo::AbaAtual));
        app.concluir_openquery();
        assert_eq!(app.workbook.len(),2);
        assert_eq!(app.workbook.valor(nova,1,0),Value::Number(42.));
        let consultas=registro::listar(&app.workbook).unwrap();
        assert_eq!(consultas[0].dados["destino"]["modo"],"aba_atual");
        assert_eq!(consultas[0].dados["destino"]["nomeAba"],"Destino");
    }

    #[test]
    fn persistir_e_atualizar_na_mesma_aba_sem_acumular_linhas() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 0, 0, "MWh".into());
        for (r, valor) in ["1","2","3"].iter().enumerate() {
            app.workbook.escrever_celula(0, (r + 1) as u32, 0, (*valor).into());
        }
        abrir_aba_para_teste(&mut app);
        app.concluir_openquery();
        assert_eq!(app.workbook.len(), 2);
        assert!(app.workbook.aba(1).unwrap().sheet.oq.is_some());
        let id = registro::listar(&app.workbook).unwrap()[0].dados["id"].as_str().unwrap().to_string();
        app.workbook.escrever_celula(0, 1, 0, "10".into());
        app.atualizar_consulta_salva(&id);
        assert_eq!(app.workbook.len(), 2);
        assert_eq!(app.workbook.valor(1, 1, 0), Value::Number(10.0));
        assert_eq!(registro::listar(&app.workbook).unwrap().len(), 1);
    }

    #[test]
    fn consulta_cria_tabela_vinculada_e_preserva_id_na_atualizacao() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 0, 0, "Id".into());
        app.workbook.escrever_celula(0, 1, 0, "123".into());
        abrir_aba_para_teste(&mut app);
        app.concluir_openquery();
        let table_before: Vec<serde_json::Value> = serde_json::from_str(
            app.workbook.aba(1).unwrap().sheet.tb.as_deref().unwrap()).unwrap();
        assert_eq!(table_before.len(),1);
        let id = table_before[0]["openQueryId"].as_str().unwrap().to_owned();
        app.workbook.escrever_celula(0, 1, 0, "456".into());
        app.atualizar_consulta_salva(&id);
        let table_after: Vec<serde_json::Value> = serde_json::from_str(
            app.workbook.aba(1).unwrap().sheet.tb.as_deref().unwrap()).unwrap();
        assert_eq!(table_after.len(),1);
        assert_eq!(table_after[0]["id"],table_before[0]["id"]);
        assert_eq!(table_after[0]["openQueryId"],id);
    }

    #[test]
    fn consulta_gravada_sobrevive_fshz_e_atualiza_apos_reabrir() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 0, 0, "Consumo".into());
        app.workbook.escrever_celula(0, 1, 0, "7".into());
        abrir_aba_para_teste(&mut app);
        app.concluir_openquery();
        let bytes = feathersheet_io::da_workbook::pasta_gravar_bytes(
            &app.workbook, app.folha as u32).unwrap();
        let reaberta = feathersheet_io::para_workbook::para_workbook(
            &feathersheet_io::fshz_read::ler_fshz(&bytes).unwrap());
        let id = registro::listar(&reaberta).unwrap()[0].dados["id"]
            .as_str().unwrap().to_string();
        assert_eq!(reaberta.aba(1).unwrap().sheet.oq.is_some(), true);
        let mut app2 = App::default();
        app2.workbook = reaberta;
        app2.workbook.escrever_celula(0, 1, 0, "21".into());
        app2.atualizar_consulta_salva(&id);
        assert_eq!(app2.workbook.len(), 2);
        assert_eq!(app2.workbook.valor(1, 1, 0), Value::Number(21.0));
    }

    #[test]
    fn fonte_removida_nunca_altera_resultado_salvo() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 0, 0, "A".into());
        app.workbook.escrever_celula(0, 1, 0, "2".into());
        abrir_aba_para_teste(&mut app);
        app.concluir_openquery();
        let id = registro::listar(&app.workbook).unwrap()[0].dados["id"]
            .as_str().unwrap().to_string();
        assert!(app.workbook.renomear_aba(0, "OrigemRenomeada"));
        let anterior = app.workbook.valor(1, 1, 0);
        app.atualizar_consulta_salva(&id);
        assert_eq!(app.workbook.valor(1, 1, 0), anterior);
        assert_eq!(app.workbook.len(), 2);
        assert!(app.status.contains("foi removida"));
    }

    #[test]
    fn crescer_resultado_nao_apaga_dados_manuais_no_destino() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 0, 0, "MWh".into());
        app.workbook.escrever_celula(0, 1, 0, "5".into());
        abrir_aba_para_teste(&mut app);
        app.concluir_openquery();
        let id = registro::listar(&app.workbook).unwrap()[0].dados["id"]
            .as_str().unwrap().to_string();
        app.workbook.escrever_celula(1, 2, 0, "NOTA".into());
        app.workbook.escrever_celula(0, 2, 0, "6".into());
        app.abrir_consulta_salva(&id);
        // A consulta guarda a origem escolhida: amplia para A1:A3 à mão,
        // como a pessoa faria no editor, para o resultado crescer.
        let area = Rect::new(0, 0, 2, 0);
        let fonte = super::ler_tabela(&app.workbook, 0, area.clone()).unwrap();
        let editor = app.openquery.as_mut().expect("editor aberto");
        editor.fonte = fonte;
        editor.origem = area;
        editor.registrar_passos();
        app.concluir_openquery();
        assert!(app.status.contains("fora da área anterior"), "{}", app.status);
        assert_eq!(app.workbook.valor(1, 2, 0), Value::Text("NOTA".into()));
        assert_eq!(app.workbook.valor(1, 1, 0), Value::Number(5.));
    }

    #[test]
    fn atualizar_limpa_sobra_sem_apagar_celula_fora_do_resultado() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 0, 0, "Número".into());
        app.workbook.escrever_celula(0, 1, 0, "11".into());
        app.workbook.escrever_celula(0, 2, 0, "12".into());
        abrir_aba_para_teste(&mut app);
        app.concluir_openquery();
        let id = registro::listar(&app.workbook).unwrap()[0].dados["id"].as_str().unwrap().to_string();
        app.workbook.escrever_celula(1, 8, 8, "Não tocar".into());
        app.abrir_consulta_salva(&id);
        app.openquery.as_mut().unwrap().passos.push(Passo::Limite { quantas: 1.0 });
        app.openquery.as_mut().unwrap().registrar_passos();
        app.concluir_openquery();
        assert_eq!(app.workbook.valor(1, 2, 0), Value::Empty);
        assert_eq!(app.workbook.valor(1, 8, 8), Value::Text("Não tocar".into()));
    }
}
