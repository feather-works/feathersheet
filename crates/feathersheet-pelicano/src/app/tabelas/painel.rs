//! Ferramentas de Tabela Iced: as mesmas ações de ui/barras/ribbon-tabela.ts
//! e ui/app/tabelas.ts, sem WebView nem estado duplicado de células.
use super::{area_de, alternar_auto_filtro, converter, reaplicar, distintos_lote, estilo, filtrar, limpar_todos_filtros, ordenar, ordenar_por, renomear, redimensionar, sob_celula, sob_qualquer, sob_id, Estilo};
use super::filtro_extra::{PAG_NUMERO, PAG_PERIODO, PAG_DATA, PAG_TOP10};
use feathersheet_core::operacoes::ordenacao_ext::CriterioOrdem;
use crate::app::{App, Msg};
use pelicano::Task;

use serde_json::json;

#[derive(Clone, Debug)]
pub enum Acao {
    Abrir, Fechar, AlternarAutoFiltro, Coluna(u32,u32),
    FiltroCabecalho(u32,u32,f32,f32), Nome(String), IniciarRenomeacao, CancelarRenomeacao, Renomear,
    IniciarRedimensionamento, CancelarRedimensionamento, Intervalo(String), Redimensionar,
    Faixas, FaixasColuna, Borda, Totais, Tema(&'static str),
    Funcao(&'static str), Ordenar(bool), Operador(&'static str),
    Criterio(String), AplicarFiltro, LimparFiltro, LimparTodosFiltros, FiltrarUnico(String),
    BuscaLista(String), MarcarValor(String), MarcarTodos(bool), AplicarLista,
    ScrollLista(f32), CarregarMais, CarregarTodos, PararCarga, ProximoLote(u64),
    AbrirCondicoes, VoltarCondicoes, FecharCondicoes,
    Converter,
    /// Seta da célula de total (linha, coluna, x, y do clique) e a conta escolhida.
    AbrirTotais(u32,u32,f32,f32), EscolherTotal(&'static str),
    /// Abre (ou fecha, se já aberto) um submenu do menu da coluna
    /// (`filtro_extra::PAG_*`).
    Submenu(u8),
    /// Filtra a coluna por um critério pronto em JSON (`filtro_json`):
    /// cor, média, período de data.
    AplicarCriterio(serde_json::Value),
    /// Formulário "10 primeiros": maiores/menores, quantidade, itens/%.
    Top10Topo(bool), Top10N(String), Top10Percentual(bool), AplicarTop10,
    /// Classifica a tabela pela coluna (cor em cima, lista personalizada).
    OrdenarPor(CriterioOrdem, bool),
    /// Texto da lista própria em "Classificar por lista".
    ListaTexto(String),
    /// Classificar e filtrar ▾ > Reaplicar: filtro e ordenação de novo
    /// sobre os dados atuais da tabela (ou AutoFiltro) da célula ativa.
    Reaplicar,
}

pub(super) const OPERADORES: &[(&str,&str)] = &[
    ("Contém","contains"), ("Igual","equals_text"), ("Começa com","starts_with"),
    ("Termina com","ends_with"), ("Não contém","not_contains"),
    ("Maior que","gt"), ("Menor que","lt"), ("Maior ou igual","gte"),
    ("Menor ou igual","lte"),
    ("Diferente de","not_equals_text"),
    ("Não começa com","not_starts_with"),
    ("Não termina com","not_ends_with"),
    ("Igual número","eq_num"), ("Diferente número","neq_num"),
    ("Verdadeiro","is_true"), ("Falso","is_false"),
    ("Está vazio","is_blank"), ("Não está vazio","is_not_blank"),
];


pub(super) const LINHAS_POR_LOTE: u32 = 8_192;

/// Representação em texto dos valores da lista, inclusive células vazias.
fn rotulo_valor(v: &serde_json::Value) -> String {
    if v.is_null() {
        "(Vazias)".into()
    } else if let Some(s) = v.as_str() {
        s.to_owned()
    } else if let Some(b) = v.as_bool() {
        b.to_string()
    } else if let Some(n) = v.as_f64() {
        n.to_string()
    } else {
        v.to_string()
    }
}

fn valor_json(texto: &str) -> serde_json::Value {
    if texto == "(Vazias)" { serde_json::Value::Null } else { json!(texto) }
}

impl App {
    /// Lê um lote pequeno da coluna. Mantém os valores distintos e o índice
    /// de busca incrementalmente, sem preparar widgets para a coluna inteira.
    fn carregar_lote_tabela(&mut self) -> Result<(), String> {
        let id = self.tabela_editando.as_deref().ok_or(tr!("Tabela não selecionada."))?;
        let tabela = sob_id(&self.workbook, self.folha, id)?;
        let area = area_de(&tabela).ok_or(tr!("Área de tabela inválida."))?;
        let col = self.tabela_col_filtro.clamp(area.c0, area.c1);
        let (novos, proxima, ultimo) = distintos_lote(
            &self.workbook, self.folha, &tabela, col,
            self.tabela_filtro_proxima_linha, LINHAS_POR_LOTE,
            &mut self.tabela_filtro_vistos,
        )?;
        let busca = self.tabela_filtro_busca.trim().to_lowercase();
        for valor in novos {
            let marcado = match self.tabela_filtro_original.as_ref() {
                Some(serde_json::Value::Array(_)) =>
                    self.tabela_filtro_criterio_valores.contains(&valor),
                Some(regra) if regra["op"] == "not_in" =>
                    !self.tabela_filtro_criterio_valores.contains(&valor),
                _ => true,
            };
            let indice = self.tabela_filtro_lista.len();
            if self.valor_bate_busca(&valor, &busca) {
                self.tabela_filtro_indices.push(indice);
            }
            if marcado { self.tabela_filtro_marcados.insert(valor.clone()); }
            self.tabela_filtro_lista.push(valor);
        }
        self.tabela_filtro_proxima_linha = proxima;
        self.tabela_filtro_ultima_linha = ultimo;
        self.tabela_filtro_truncado = proxima <= ultimo;
        if !self.tabela_filtro_truncado { self.tabela_filtro_automatico = false; }
        Ok(())
    }

    fn agendar_lote_tabela(&self) -> Task<Msg> {
        if !self.tabela_filtro_automatico || !self.tabela_filtro_truncado {
            return Task::none();
        }
        let geracao = self.tabela_filtro_geracao;
        Task::perform(async move {
            tokio::time::sleep(std::time::Duration::from_millis(16)).await;
        }, move |_| Msg::TabelaAcao(Acao::ProximoLote(geracao)))
    }

    /// Reinicia a varredura a cada troca de coluna/tabela.
    fn carregar_opcoes_tabela(&mut self) {
        self.tabela_filtro_geracao = self.tabela_filtro_geracao.wrapping_add(1);
        self.tabela_filtro_automatico = false;
        self.tabela_filtro_editado = false;
        self.tabela_filtro_pagina = 0;
        self.tabela_filtro_busca.clear();
        self.tabela_filtro_lista.clear();
        self.tabela_filtro_vistos.clear();
        self.tabela_filtro_criterio_valores.clear();
        self.tabela_filtro_indices.clear();
        self.tabela_filtro_marcados.clear();
        self.tabela_filtro_original = None;
        self.tabela_filtro_inicio_visivel = 0;
        self.tabela_filtro_truncado = false;
        let Some(id) = self.tabela_editando.as_deref() else { return };
        let Ok(t) = sob_id(&self.workbook, self.folha, id) else { return };
        let Some(area) = area_de(&t) else { return };
        let col = self.tabela_col_filtro.clamp(area.c0, area.c1);
        let rel = col - area.c0;
        self.tabela_filtro_original = t["filtros"].as_array()
            .and_then(|fs| fs.iter().find(|f| f["col"].as_u64() == Some(rel as u64)))
            .map(|f| f["criterio"].clone());
        if let Some(criterio) = self.tabela_filtro_original.as_ref() {
            let lista = match criterio {
                serde_json::Value::Array(lista) => Some(lista),
                regra if regra["op"] == "not_in" => regra["val"].as_array(),
                _ => None,
            };
            if let Some(lista) = lista {
                self.tabela_filtro_criterio_valores =
                    lista.iter().map(rotulo_valor).collect();
            }
        }
        self.tabela_filtro_proxima_linha = area.r0.saturating_add(u32::from(t["temCabecalho"] == true));
        self.tabela_filtro_ultima_linha = area.r1;
        if let Err(erro) = self.carregar_lote_tabela() { self.status = erro; }
        self.analisar_coluna_filtro();
    }

    pub(in crate::app) fn acao_tabela(&mut self, acao: Acao) -> Task<Msg> {
        if let Some(tarefa) = self.acao_totais(&acao) { return tarefa; }
        if matches!(acao, Acao::Fechar) {
            self.tabela_popup_filtro = false;
            self.tabela_filtro_pagina = 0;
            self.tabela_filtro_automatico = false;
            self.tabela_filtro_geracao = self.tabela_filtro_geracao.wrapping_add(1);
            return Task::none();
        }
        if matches!(acao, Acao::IniciarRedimensionamento | Acao::CancelarRedimensionamento) {
            self.tabela_redimensionando =
                matches!(acao, Acao::IniciarRedimensionamento) && self.tabela_editando.is_some();
            if self.tabela_redimensionando {
                self.tabela_renomeando = false;
                if let Some(area) = self.tabela_editando.as_deref()
                    .and_then(|id|sob_id(&self.workbook,self.folha,id).ok())
                    .and_then(|t|area_de(&t))
                {
                    self.tabela_intervalo_novo =
                        feathersheet_core::celulas::refs::format_ref(&area);
                }
            }
            return Task::none();
        }
        if matches!(acao, Acao::IniciarRenomeacao | Acao::CancelarRenomeacao) {
            if let Some(id) = self.tabela_editando.as_deref() {
                if let Ok(t) = sob_id(&self.workbook, self.folha, id) {
                    self.tabela_nome_novo = t["nome"].as_str().unwrap_or("").to_owned();
                }
            }
            self.tabela_renomeando = matches!(acao, Acao::IniciarRenomeacao)
                && self.tabela_editando.is_some();
            if self.tabela_renomeando { self.tabela_redimensionando = false; }
            return Task::none();
        }
        if matches!(acao, Acao::Reaplicar) {
            let Some(id) = sob_qualquer(&self.workbook, self.folha, self.linha, self.coluna)
                .and_then(|t| t["id"].as_str().map(str::to_owned)) else {
                self.status = tr!("Reaplicar: selecione uma célula de uma tabela ou de um intervalo com filtro.").into();
                return Task::none();
            };
            match reaplicar(&mut self.workbook, self.folha, &id) {
                Ok((ordenou, ocultas, filtro)) => {
                    self.alterado = true;
                    let mut partes = Vec::new();
                    if ordenou { partes.push(tr!("classificação refeita").to_string()); }
                    if filtro { partes.push(trf!("filtro reaplicado ({ocultas} linha(s) oculta(s))")); }
                    if partes.is_empty() { partes.push(tr!("nada mudou").into()); }
                    self.status = trf!("Reaplicar: {}.", partes.join("; "));
                }
                Err(e) => self.status = e,
            }
            return Task::none();
        }
        if matches!(acao, Acao::AlternarAutoFiltro) {
            if sob_celula(&self.workbook,self.folha,self.linha,self.coluna).is_some() {
                return self.acao_tabela(Acao::Abrir);
            }
            let selecao = self.area_selecionada();
            match alternar_auto_filtro(
                &mut self.workbook,self.folha,selecao,self.linha,self.coluna)
            {
                Ok(s)=>{self.status=s;self.alterado=true;}
                Err(e)=>self.status=e,
            }
            return Task::none();
        }
        if matches!(acao, Acao::Abrir) {
            self.tabela_popup_filtro = false;
            self.tabela_renomeando = false;
            self.tabela_redimensionando = false;
            if let Some(t) = sob_celula(&self.workbook,self.folha,self.linha,self.coluna) {
                self.tabela_editando=t["id"].as_str().map(str::to_owned);
                self.tabela_nome_novo=t["nome"].as_str().unwrap_or("").to_owned();
                self.tabela_operador="contains";
                self.tabela_criterio.clear();
                self.tabela_col_filtro=self.coluna;
                self.carregar_opcoes_tabela();
                self.faixa_ativa = crate::app::AbaFaixa::Tabela;
            } else {
                self.status=tr!("Selecione uma célula dentro de uma tabela criada.").into();
            }
            return Task::none();
        }
        if let Acao::Coluna(r,c) | Acao::FiltroCabecalho(r,c,_,_) = acao {
            let clique_x = match acao {
                Acao::FiltroCabecalho(_,_,x,_) => x,
                _ => (self.tamanho_principal.width * 0.28).max(0.0),
            };
            self.trocar_selecao(r,c);
            self.tabela_col_filtro=c;
            self.tabela_editando=sob_qualquer(&self.workbook,self.folha,r,c)
                .and_then(|t|t["id"].as_str().map(str::to_owned));
            self.tabela_nome_novo=self.tabela_editando.as_ref()
                .and_then(|id|sob_id(&self.workbook,self.folha,id).ok())
                .and_then(|t|t["nome"].as_str().map(str::to_owned)).unwrap_or_default();
            self.carregar_opcoes_tabela();
            self.tabela_popup_x = (clique_x - 18.0).max(0.0);
            self.tabela_popup_y = match acao {
                Acao::FiltroCabecalho(_,_,_,y) => y.max(0.0),
                _ => 0.0,
            };
            self.tabela_popup_filtro = self.tabela_editando.is_some();
            return Task::none();
        }
        let Some(id)=self.tabela_editando.clone() else { return Task::none() };
        let t=match sob_id(&self.workbook,self.folha,&id) {
            Ok(t)=>t,
            Err(e)=>{self.status=e;self.tabela_editando=None;return Task::none();}
        };
        let Some(area)=area_de(&t) else {self.status=tr!("Área de tabela inválida.").into();return Task::none()};
        let col=self.tabela_col_filtro.clamp(area.c0,area.c1);
        let rel=col-area.c0;
        let fechar_popup_apos=matches!(acao,Acao::AplicarLista|Acao::AplicarFiltro
            |Acao::FiltrarUnico(_)|Acao::LimparFiltro|Acao::LimparTodosFiltros|Acao::Ordenar(_)
            |Acao::AplicarCriterio(_)|Acao::AplicarTop10|Acao::OrdenarPor(_,_));
        let confirmou_renomeacao = matches!(acao, Acao::Renomear);
        let confirmou_redimensionamento = matches!(acao, Acao::Redimensionar);
        let resultado: Result<String,String>=match acao {
            Acao::Nome(s)=>{self.tabela_nome_novo=s;return Task::none();}
            Acao::Intervalo(s)=>{self.tabela_intervalo_novo=s;return Task::none();}
            Acao::Redimensionar=>{
                feathersheet_core::celulas::refs::parse_range_ref(&self.tabela_intervalo_novo)
                    .ok_or_else(||tr!("Intervalo inválido. Use A1:C20, por exemplo.").to_string())
                    .and_then(|nova|redimensionar(&mut self.workbook,self.folha,&id,nova))
                    .map(|_|tr!("Intervalo alterado sem apagar células.").into())
            },
            Acao::BuscaLista(s) => {
                self.tabela_filtro_busca = s;
                self.tabela_filtro_inicio_visivel = 0;
                self.tabela_filtro_indices.clear();
                let busca = self.tabela_filtro_busca.trim().to_lowercase();
                let achados: Vec<usize> = self.tabela_filtro_lista.iter().enumerate()
                    .filter(|(_, valor)| self.valor_bate_busca(valor, &busca))
                    .map(|(i, _)| i).collect();
                self.tabela_filtro_indices.extend(achados);
                return Task::snap_to(
                    "feathersheet-filtro-valores",
                    pelicano::Point::new(0.0, 0.0),
                );
            },
            Acao::ScrollLista(y) => {
                let max = self.tabela_filtro_indices.len().saturating_sub(1);
                self.tabela_filtro_inicio_visivel = ((y.max(0.0) / 26.0) as usize).min(max);
                return Task::none();
            },
            Acao::CarregarMais => {
                self.tabela_filtro_automatico = false;
                if let Err(erro) = self.carregar_lote_tabela() { self.status = erro; }
                return Task::none();
            },
            Acao::PararCarga => {
                self.tabela_filtro_automatico = false;
                self.tabela_filtro_geracao = self.tabela_filtro_geracao.wrapping_add(1);
                return Task::none();
            },
            Acao::CarregarTodos => {
                self.tabela_filtro_automatico = true;
                if let Err(erro) = self.carregar_lote_tabela() {
                    self.status = erro;
                    self.tabela_filtro_automatico = false;
                }
                return self.agendar_lote_tabela();
            },
            Acao::ProximoLote(geracao) => {
                if geracao != self.tabela_filtro_geracao
                    || !self.tabela_popup_filtro || !self.tabela_filtro_automatico
                { return Task::none(); }
                if let Err(erro) = self.carregar_lote_tabela() {
                    self.status = erro;
                    self.tabela_filtro_automatico = false;
                }
                return self.agendar_lote_tabela();
            },
            Acao::MarcarValor(v) => {
                if !self.tabela_filtro_marcados.remove(&v)
                    && self.tabela_filtro_vistos.contains(&v)
                {
                    self.tabela_filtro_marcados.insert(v);
                }
                self.tabela_filtro_editado = true;
                return Task::none();
            },
            Acao::MarcarTodos(marcar) => {
                // Selecionar tudo/Limpar afetam todos os valores encontrados
                // pela pesquisa, não só os widgets renderizados.
                for &indice in &self.tabela_filtro_indices {
                    let valor = &self.tabela_filtro_lista[indice];
                    if marcar {
                        if self.tabela_filtro_marcados.insert(valor.clone()) {
                            self.tabela_filtro_editado = true;
                        }
                    } else if self.tabela_filtro_marcados.remove(valor) {
                        self.tabela_filtro_editado = true;
                    }
                }
                return Task::none();
            },
            Acao::AbrirCondicoes => {
                // Flyout sobreposto; o menu principal e seus valores permanecem.
                self.tabela_filtro_pagina = if self.tabela_filtro_pagina == 0 { 1 } else { 0 };
                self.tabela_filtro_extra.voltar = 1;
                return Task::none();
            },
            Acao::Submenu(p) => {
                self.tabela_filtro_pagina = if self.tabela_filtro_pagina == p { 0 } else { p };
                if p == PAG_NUMERO { self.tabela_filtro_extra.voltar = PAG_NUMERO; }
                return Task::none();
            },
            Acao::Top10Topo(v) => { self.tabela_filtro_extra.top_topo = v; return Task::none(); },
            Acao::Top10Percentual(v) => { self.tabela_filtro_extra.top_percentual = v; return Task::none(); },
            Acao::Top10N(v) => {
                // Só algarismos; o Excel aceita de 1 a 500.
                self.tabela_filtro_extra.top_n = v.chars().filter(char::is_ascii_digit).take(3).collect();
                return Task::none();
            },
            Acao::ListaTexto(v) => { self.tabela_filtro_extra.lista_texto = v; return Task::none(); },
            Acao::FecharCondicoes => {
                self.tabela_filtro_pagina = 0;
                return Task::none();
            },
            Acao::VoltarCondicoes => {
                self.tabela_filtro_pagina = match self.tabela_filtro_pagina {
                    2 => self.tabela_filtro_extra.voltar,
                    PAG_TOP10 => PAG_NUMERO,
                    PAG_PERIODO => PAG_DATA,
                    _ => 0,
                };
                return Task::none();
            },
            Acao::AplicarLista => {
                // Abrir e confirmar uma lista incompleta não limpa filtros já
                // ativos. O usuário precisa ter efetivamente mudado a seleção.
                if !self.tabela_filtro_editado {
                    self.tabela_popup_filtro = false;
                    self.tabela_filtro_automatico = false;
                    self.tabela_filtro_geracao = self.tabela_filtro_geracao.wrapping_add(1);
                    return Task::none();
                }
                let marcados = &self.tabela_filtro_marcados;
                let criterio = if self.tabela_filtro_truncado {
                    match self.tabela_filtro_original.as_ref() {
                        Some(serde_json::Value::Array(antigos)) => {
                            // Conserva valores permitidos antes, mas ainda
                            // não descobertos nesta varredura.
                            let mut lista: Vec<_> = antigos.iter().filter(|v| {
                                !self.tabela_filtro_vistos.contains(&rotulo_valor(v))
                            }).cloned().collect();
                            lista.extend(self.tabela_filtro_lista.iter()
                                .filter(|v| marcados.contains(*v)).map(|v| valor_json(v)));
                            Some(json!(lista))
                        },
                        Some(regra) if regra["op"] == "not_in" => {
                            // Mantém exclusões antigas fora da amostra.
                            let mut removidos: Vec<_> = regra["val"].as_array()
                                .into_iter().flatten().filter(|v| {
                                    !self.tabela_filtro_vistos.contains(&rotulo_valor(v))
                                }).cloned().collect();
                            removidos.extend(self.tabela_filtro_lista.iter()
                                .filter(|v| !marcados.contains(*v)).map(|v| valor_json(v)));
                            Some(json!({"op":"not_in","val":removidos}))
                        },
                        Some(_) => {
                            self.status = tr!("Carregue todos os valores antes de trocar uma condição por uma lista.").into();
                            return Task::none();
                        },
                        None => {
                            let removidos: Vec<_> = self.tabela_filtro_lista.iter()
                                .filter(|v| !marcados.contains(*v)).map(|v| valor_json(v)).collect();
                            Some(json!({"op":"not_in","val":removidos}))
                        },
                    }
                } else if marcados.len() == self.tabela_filtro_lista.len() {
                    None
                } else {
                    let desmarcados = self.tabela_filtro_lista.len() - marcados.len();
                    if desmarcados < marcados.len() {
                        // NOT_IN ocupa menos espaço quando quase tudo foi marcado.
                        let removidos: Vec<_> = self.tabela_filtro_lista.iter()
                            .filter(|v| !marcados.contains(*v)).map(|v| valor_json(v)).collect();
                        Some(json!({"op":"not_in","val":removidos}))
                    } else {
                        Some(json!(self.tabela_filtro_lista.iter()
                            .filter(|v| marcados.contains(*v))
                            .map(|v| valor_json(v)).collect::<Vec<_>>()))
                    }
                };
                filtrar(&mut self.workbook, self.folha, &id, rel, criterio)
                    .map(|n| trf!("Filtro aplicado: {n} linhas ocultadas."))
            },
            Acao::Criterio(s)=>{self.tabela_criterio=s;return Task::none();}
            Acao::Operador(op)=>{
                self.tabela_operador=op;
                self.tabela_criterio.clear();
                self.tabela_filtro_pagina=2;
                return Task::none();
            }
            Acao::Renomear=>renomear(&mut self.workbook,self.folha,&id,&self.tabela_nome_novo)
                .map(|_|trf!("Tabela renomeada para {}.",self.tabela_nome_novo)),
            Acao::Faixas=>estilo(&mut self.workbook,self.folha,&id,Estilo::Faixas(t["faixas"]!=true))
                .map(|_|tr!("Faixas por linha atualizadas.").into()),
            Acao::FaixasColuna=>estilo(&mut self.workbook,self.folha,&id,
                    Estilo::FaixasColuna(t["estilo"]["faixasColuna"]!=true))
                .map(|_|tr!("Faixas por coluna atualizadas.").into()),
            Acao::Borda=>estilo(&mut self.workbook,self.folha,&id,Estilo::Borda(t["estilo"]["borda"]!=true))
                .map(|_|tr!("Borda da tabela atualizada.").into()),
            Acao::Totais=>{
                let ligar=t["mostrarTotais"]!=true;
                super::totais::alternar_totais(&mut self.workbook,self.folha,&id,ligar)
                    .map(|_|if ligar {tr!("Linha de totais ligada: clique numa célula dela para escolher o cálculo.")}
                        else {tr!("Linha de totais removida.")}.into())
            },
            Acao::Tema(tema)=>estilo(&mut self.workbook,self.folha,&id,Estilo::Tema(tema.into()))
                .map(|_|trf!("Paleta {tema} aplicada.")),
            Acao::Funcao(funcao)=>{
                // Uma ação só: liga a linha (se preciso) e grava a fórmula.
                let ligada=t["mostrarTotais"]==true;
                if let Some(a)=self.workbook.aba_mut(self.folha) { a.sheet.abrir_transacao(); }
                let resultado=(if ligada {Ok(())} else {
                        super::totais::alternar_totais(&mut self.workbook,self.folha,&id,true)})
                    .and_then(|_|super::totais::definir_total(&mut self.workbook,self.folha,&id,rel,funcao));
                if let Some(a)=self.workbook.aba_mut(self.folha) {
                    if resultado.is_ok() { a.sheet.fechar_transacao(); }
                    else { a.sheet.reverter_transacao(); }
                }
                resultado.map(|_|trf!("Total {} configurado para a coluna {}.",funcao,rel+1))
            },
            Acao::Ordenar(asc)=>ordenar(&mut self.workbook,self.folha,&id,rel,asc)
                .map(|_|if asc{tr!("Tabela ordenada A → Z").into()}else{tr!("Tabela ordenada Z → A").into()}),
            Acao::OrdenarPor(criterio,asc)=>{
                let rotulo=match &criterio {
                    CriterioOrdem::Cor{..}=>tr!("Tabela classificada pela cor escolhida."),
                    CriterioOrdem::Lista(_)=>tr!("Tabela classificada pela lista."),
                    CriterioOrdem::Valor=>tr!("Tabela classificada."),
                };
                ordenar_por(&mut self.workbook,self.folha,&id,rel,criterio,asc)
                    .map(|mudou|if mudou {rotulo.into()} else {tr!("A ordem já era essa.").into()})
            },
            Acao::AplicarCriterio(criterio)=>filtrar(&mut self.workbook,self.folha,&id,rel,Some(criterio))
                .map(|q|trf!("Filtro aplicado: {q} linhas ocultadas.")),
            Acao::AplicarTop10=>{
                let e=&self.tabela_filtro_extra;
                match e.top_n.parse::<u32>() {
                    Ok(n) if (1..=500).contains(&n) => {
                        let crit=json!({"op":"top10","topo":e.top_topo,"n":n,"percentual":e.top_percentual});
                        filtrar(&mut self.workbook,self.folha,&id,rel,Some(crit))
                            .map(|q|trf!("Filtro aplicado: {q} linhas ocultadas."))
                    },
                    _ => Err(tr!("Informe um número de 1 a 500.").into()),
                }
            },
            Acao::AplicarFiltro=>{
                let valor=self.tabela_criterio.trim();
                let sem_parametro=["is_true","is_false","is_blank","is_not_blank"]
                    .contains(&self.tabela_operador);
                if valor.is_empty() && !sem_parametro {
                    Err(tr!("Informe um valor para o critério.").into())
                } else {
                    // Data digitada (15/10/2026) vira o serial, como na célula;
                    // "Igual"/"Diferente" numa data compara como número.
                    use feathersheet_core::celulas::{input::{parse_literal, NumberFormat}, values::Value};
                    let literal=parse_literal(valor);
                    let data=matches!(literal.format,Some(NumberFormat::Date|NumberFormat::Datetime));
                    let operador=match (self.tabela_operador,data) {
                        ("equals_text",true)=>"eq_num",
                        ("not_equals_text",true)=>"neq_num",
                        (op,_)=>op,
                    };
                    let regra=if sem_parametro {
                        Ok(json!({"op":operador,"val":null}))
                    } else if ["gt","lt","gte","lte","eq_num","neq_num"].contains(&operador) {
                        let numero=valor.replace(',', ".").parse::<f64>().ok()
                            .or(match literal.value { Value::Number(n) => Some(n), _ => None })
                            .ok_or_else(||tr!("Informe um número ou uma data válida para comparação.").to_string());
                        numero.map(|n|json!({"op":operador,"val":n}))
                    } else {Ok(json!({"op":operador,"val":valor}))};
                    regra.and_then(|crit|filtrar(&mut self.workbook,self.folha,&id,rel,Some(crit)))
                        .map(|q|trf!("Filtro aplicado: {q} linhas ocultadas."))
                }
            },
            Acao::FiltrarUnico(s)=> {
                let valor=if s=="(Vazias)" {serde_json::Value::Null} else {json!(s)};
                filtrar(&mut self.workbook,self.folha,&id,rel,Some(json!([valor])))
                    .map(|q|trf!("Filtro aplicado: {q} linhas ocultadas."))
            },
            Acao::LimparFiltro=>filtrar(&mut self.workbook,self.folha,&id,rel,None)
                .map(|_|tr!("Filtro da coluna removido.").into()),
            Acao::LimparTodosFiltros=>limpar_todos_filtros(&mut self.workbook,self.folha,&id)
                .map(|n|trf!("{n} filtros removidos da tabela.")),
            Acao::Converter=>{
                let res=converter(&mut self.workbook,self.folha,&id)
                    .map(|_|tr!("Tabela convertida em intervalo, sem apagar as células.").into());
                if res.is_ok() {
                    self.tabela_editando=None;
                    self.tabela_renomeando=false;
                    self.tabela_redimensionando=false;
                    self.tabela_popup_filtro=false;
                    self.faixa_ativa=crate::app::AbaFaixa::Inicio;
                }
                res
            },
            Acao::IniciarRenomeacao|Acao::CancelarRenomeacao
                |Acao::Abrir|Acao::Fechar|Acao::AlternarAutoFiltro|Acao::Reaplicar
                |Acao::Coluna(_,_)|Acao::FiltroCabecalho(_,_,_,_)
                |Acao::IniciarRedimensionamento|Acao::CancelarRedimensionamento
                |Acao::AbrirTotais(_,_,_,_)|Acao::EscolherTotal(_)=>unreachable!(),
        };
        if fechar_popup_apos && resultado.is_ok() {
            self.tabela_popup_filtro = false;
            self.tabela_filtro_pagina = 0;
            self.tabela_filtro_automatico = false;
            self.tabela_filtro_geracao = self.tabela_filtro_geracao.wrapping_add(1);
        }
        if confirmou_renomeacao && resultado.is_ok() {
            self.tabela_renomeando = false;
        }
        if confirmou_redimensionamento && resultado.is_ok() {
            self.tabela_redimensionando = false;
            if let Ok(t) = sob_id(&self.workbook,self.folha,&id) {
                if let Some(area) = area_de(&t) {
                    if !area.contains(self.linha,self.coluna) {
                        self.trocar_selecao(area.r0,area.c0);
                    }
                }
            }
        }
        match resultado{
            Ok(s)=>{self.status=s;self.alterado=true;}
            Err(e)=>self.status=e,
        }
        Task::none()
    }


}

#[cfg(test)]
mod testes {
    use super::*;
    use pelicano::Element;
    use feathersheet_core::celulas::refs::Rect;

    #[test]
    fn design_da_tabela_fica_na_ribbon_e_filtro_no_cabecalho() {
        use crate::app::AbaFaixa;
        let mut app=App::default();
        app.workbook.escrever_celula(0,0,0,"Nome".into());
        app.workbook.escrever_celula(0,0,1,"Valor".into());
        app.workbook.escrever_celula(0,1,0,"A".into());
        app.workbook.escrever_celula(0,1,1,"2".into());
        app.workbook.escrever_celula(0,2,0,"B".into());
        app.workbook.escrever_celula(0,2,1,"4".into());
        super::super::criar(
            &mut app.workbook,0,Rect::new(0,0,2,1),"Tabela1",true,false
        ).unwrap();
        app.trocar_selecao(0,0);
        let _=app.update(Msg::AbaFaixa(AbaFaixa::Tabela));
        assert_eq!(app.faixa_ativa,AbaFaixa::Tabela);
        assert!(!app.tabela_popup_filtro);
        {
            let _faixa: Element<'_,Msg> = app.view_faixa();
        }
        let _=app.acao_tabela(Acao::Tema("verde"));
        let _=app.acao_tabela(Acao::FaixasColuna);
        let t=super::sob_id(&app.workbook,0,"tab_Tabela1").unwrap();
        assert_eq!(t["estilo"]["tema"],"verde");
        assert_eq!(t["estilo"]["faixasColuna"],true);

        // O clique da setinha do cabeçalho abre SOMENTE o menu compacto.
        let _=app.acao_tabela(Acao::FiltroCabecalho(0,1,440.0,20.0));
        assert!(app.tabela_popup_filtro);
        assert_eq!(app.tabela_popup_x,422.0);
        assert_eq!(app.tabela_col_filtro,1);
        assert_eq!(app.tabela_filtro_lista.len(),2);
        {
            let _menu: Element<'_,Msg> = app.menu_filtro_tabela();
        }
        let _=app.acao_tabela(Acao::MarcarTodos(false));
        let _=app.acao_tabela(Acao::MarcarValor("2".into()));
        let _=app.acao_tabela(Acao::AplicarLista);
        assert!(!app.tabela_popup_filtro);
        let t=super::sob_id(&app.workbook,0,"tab_Tabela1").unwrap();
        assert_eq!(t["filtros"][0]["col"],1);
        assert_eq!(t["filtros"][0]["criterio"][0],"2");
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(1,1),"2");
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(2,1),"4");

        let _=app.acao_tabela(Acao::FiltroCabecalho(0,1,440.0,20.0));
        let _=app.update(Msg::Cancelar);
        assert!(!app.tabela_popup_filtro);
        app.trocar_selecao(8,8);
        assert_eq!(app.faixa_ativa,AbaFaixa::Inicio);
        assert!(app.tabela_editando.is_none());
    }

    #[test]
    fn abrir_condicoes_mantem_o_menu_principal_e_a_lista_marcada() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 0, 0, "Item".into());
        app.workbook.escrever_celula(0, 1, 0, "A".into());
        app.workbook.escrever_celula(0, 2, 0, "B".into());
        super::super::criar(&mut app.workbook, 0, Rect::new(0, 0, 2, 0),
            "Tabela1", true, false).unwrap();
        let _ = app.acao_tabela(Acao::FiltroCabecalho(0, 0, 140.0, 18.0));
        assert!(app.tabela_popup_filtro);
        let marcados = app.tabela_filtro_marcados.clone();
        let _ = app.acao_tabela(Acao::AbrirCondicoes);
        assert_eq!(app.tabela_filtro_pagina, 1);
        assert!(app.tabela_popup_filtro);
        assert_eq!(app.tabela_filtro_marcados, marcados);
        let _principal: Element<'_, Msg> = app.menu_filtro_tabela();
        let _sub: Element<'_, Msg> = app.menu_condicoes_tabela();
        drop(_principal);
        drop(_sub);

        let _ = app.acao_tabela(Acao::Operador("contains"));
        assert_eq!(app.tabela_filtro_pagina, 2);
        let _ = app.acao_tabela(Acao::VoltarCondicoes);
        assert_eq!(app.tabela_filtro_pagina, 1);
        let _ = app.acao_tabela(Acao::FecharCondicoes);
        assert_eq!(app.tabela_filtro_pagina, 0);
        assert!(app.tabela_popup_filtro, "fechar o submenu não fecha o principal");
        assert_eq!(app.tabela_filtro_marcados, marcados);
    }

    #[test]
    fn converter_via_ribbon_nao_remove_celulas() {
        use crate::app::AbaFaixa;
        let mut app=App::default();
        app.workbook.escrever_celula(0,0,0,"Nome".into());
        app.workbook.escrever_celula(0,1,0,"Teste".into());
        super::super::criar(
            &mut app.workbook,0,Rect::new(0,0,1,0),"Tabela1",true,false
        ).unwrap();
        app.trocar_selecao(0,0);
        let _=app.acao_tabela(Acao::Abrir);
        assert_eq!(app.faixa_ativa,AbaFaixa::Tabela);
        let _=app.acao_tabela(Acao::Converter);
        assert_eq!(app.faixa_ativa,AbaFaixa::Inicio);
        assert!(!app.tabela_popup_filtro);
        assert!(super::sob_celula(&app.workbook,0,0,0).is_none());
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(1,0),"Teste");
    }

    #[test]
    fn redimensionamento_disponivel_na_ribbon_tabela() {
        let mut app=App::default();
        app.workbook.escrever_celula(0,0,0,"Produto".into());
        app.workbook.escrever_celula(0,1,0,"Caneta".into());
        super::super::criar(&mut app.workbook,0,Rect::new(0,0,1,0),"Tabela1",true,false).unwrap();
        let _=app.acao_tabela(Acao::Abrir);
        let _=app.acao_tabela(Acao::IniciarRedimensionamento);
        assert!(app.tabela_redimensionando);
        assert_eq!(app.tabela_intervalo_novo,"A1:A2");
        let _=app.acao_tabela(Acao::Intervalo("A1:C10".into()));
        let _=app.acao_tabela(Acao::Redimensionar);
        assert!(!app.tabela_redimensionando);
        assert_eq!(super::sob_id(&app.workbook,0,"tab_Tabela1").unwrap()["area"]["c1"],2);
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(1,0),"Caneta");
    }

    #[test]
    fn reabrir_filtro_not_in_nao_seleciona_valores_vazios_novamente() {
        let mut app = App::default();
        app.workbook.escrever_celula(0,0,0,"Item".into());
        app.workbook.escrever_celula(0,1,0,"A".into());
        // Linha 2 propositalmente vazia.
        app.workbook.escrever_celula(0,3,0,"B".into());
        super::super::criar(&mut app.workbook,0,Rect::new(0,0,3,0),
            "Tabela1",true,false).unwrap();
        super::filtrar(&mut app.workbook,0,"tab_Tabela1",0,
            Some(json!({"op":"not_in","val":[null]}))).unwrap();
        let _=app.acao_tabela(Acao::FiltroCabecalho(0,0,100.0,24.0));
        assert!(app.tabela_filtro_lista.contains(&"(Vazias)".to_string()));
        assert!(!app.tabela_filtro_marcados.contains(&"(Vazias)".to_string()));
        assert!(app.tabela_filtro_marcados.contains(&"A".to_string()));
        assert!(app.tabela_filtro_marcados.contains(&"B".to_string()));
    }

    #[test]
    fn total_de_coluna_e_totais_acionados_com_um_unico_undo() {
        let mut app=App::default();
        app.workbook.escrever_celula(0,0,0,"Valor".into());
        app.workbook.escrever_celula(0,1,0,"12".into());
        app.workbook.escrever_celula(0,2,0,"15".into());
        super::super::criar(&mut app.workbook,0,Rect::new(0,0,2,0),
            "Tabela1",true,false).unwrap();
        let _=app.acao_tabela(Acao::Abrir);
        let _=app.acao_tabela(Acao::Funcao("soma"));
        let t=super::sob_id(&app.workbook,0,"tab_Tabela1").unwrap();
        assert_eq!(t["mostrarTotais"],true);
        assert_eq!(t["totais"][0]["fn"],"soma");
        let _=app.update(Msg::Desfazer);
        let t=super::sob_id(&app.workbook,0,"tab_Tabela1").unwrap();
        assert_ne!(t["mostrarTotais"],true);
        assert!(t["totais"].as_array().is_none_or(|a|a.is_empty()));
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(1,0),"12");
        let _=app.update(Msg::Refazer);
        let t=super::sob_id(&app.workbook,0,"tab_Tabela1").unwrap();
        assert_eq!(t["mostrarTotais"],true);
        assert_eq!(t["totais"][0]["fn"],"soma");
    }

    #[test]
    fn popup_limpa_filtros_de_mais_de_uma_coluna() {
        let mut app=App::default();
        for (r,a,b) in [(0,"Tipo","Numero"),(1,"A","10"),(2,"B","20"),(3,"A","30")] {
            app.workbook.escrever_celula(0,r,0,a.into());
            app.workbook.escrever_celula(0,r,1,b.into());
        }
        super::super::criar(&mut app.workbook,0,Rect::new(0,0,3,1),
            "Tabela1",true,false).unwrap();
        super::filtrar(&mut app.workbook,0,"tab_Tabela1",0,Some(json!(["A"]))).unwrap();
        super::filtrar(&mut app.workbook,0,"tab_Tabela1",1,
            Some(json!({"op":"gt","val":15}))).unwrap();
        let _=app.acao_tabela(Acao::FiltroCabecalho(0,0,140.0,30.0));
        assert!(app.tabela_popup_filtro);
        let _popup: Element<'_,Msg> = app.menu_filtro_tabela();
        drop(_popup);
        let _=app.acao_tabela(Acao::LimparTodosFiltros);
        assert!(!app.tabela_popup_filtro);
        assert!(app.status.contains("2 filtros removidos"),"{}",app.status);
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(2,0),"B");
        assert!(super::sob_id(&app.workbook,0,"tab_Tabela1").unwrap()["filtros"]
            .as_array().unwrap().is_empty());
    }

    #[test]
    fn popup_filtro_esta_vazio_sem_parametro_digitado() {
        let mut app=App::default();
        app.workbook.escrever_celula(0,0,0,"Tipo".into());
        app.workbook.escrever_celula(0,1,0,"A".into());
        // A3 permanece vazia, mas esta dentro da area com dados ate A4.
        app.workbook.escrever_celula(0,3,0,"B".into());
        super::super::criar(&mut app.workbook,0,Rect::new(0,0,3,0),
            "Tabela1",true,false).unwrap();
        let _=app.acao_tabela(Acao::FiltroCabecalho(0,0,100.0,20.0));
        let _=app.acao_tabela(Acao::Operador("is_blank"));
        let _flyout: Element<'_,Msg> = app.menu_condicoes_tabela();
        drop(_flyout);
        let _=app.acao_tabela(Acao::AplicarFiltro);
        assert!(app.status.contains("Filtro aplicado"),"{}",app.status);
        assert_eq!(super::sob_id(&app.workbook,0,"tab_Tabela1").unwrap()
            ["filtros"][0]["criterio"]["op"],"is_blank");
        assert_eq!(app.workbook.aba(0).unwrap().sheet.row_heights.get(&1),Some(&0));
        assert_ne!(app.workbook.aba(0).unwrap().sheet.row_heights.get(&2),Some(&0));
    }

    #[test]
    fn criar_e_renomear_tabela_preservam_referencias_em_undo_redo() {
        let mut app=App::default();
        app.workbook.escrever_celula(0,0,0,"Produto".into());
        app.workbook.escrever_celula(0,1,0,"Caneta".into());
        super::super::criar(&mut app.workbook,0,Rect::new(0,0,1,0),
            "Tabela1",true,false).unwrap();
        assert!(app.workbook.nomeados().iter().any(|n|n.nome=="Tabela1"));
        let _=app.update(Msg::Desfazer);
        assert!(super::super::sob_celula(&app.workbook,0,0,0).is_none());
        assert!(!app.workbook.nomeados().iter().any(|n|n.nome=="Tabela1"));
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(1,0),"Caneta");
        let _=app.update(Msg::Refazer);
        assert_eq!(super::sob_id(&app.workbook,0,"tab_Tabela1").unwrap()["nome"],"Tabela1");
        assert!(app.workbook.nomeados().iter().any(|n|n.nome=="Tabela1"));
        let _=app.acao_tabela(Acao::Abrir);
        let _=app.acao_tabela(Acao::Nome("Estoque".into()));
        let _=app.acao_tabela(Acao::Renomear);
        assert_eq!(super::sob_id(&app.workbook,0,"tab_Tabela1").unwrap()["nome"],"Estoque");
        assert!(app.workbook.nomeados().iter().any(|n|n.nome=="Estoque"));
        assert!(!app.workbook.nomeados().iter().any(|n|n.nome=="Tabela1"));
        let _=app.update(Msg::Desfazer);
        assert_eq!(super::sob_id(&app.workbook,0,"tab_Tabela1").unwrap()["nome"],"Tabela1");
        assert!(app.workbook.nomeados().iter().any(|n|n.nome=="Tabela1"));
        assert!(!app.workbook.nomeados().iter().any(|n|n.nome=="Estoque"));
        let _=app.update(Msg::Refazer);
        assert_eq!(super::sob_id(&app.workbook,0,"tab_Tabela1").unwrap()["nome"],"Estoque");
        assert!(app.workbook.nomeados().iter().any(|n|n.nome=="Estoque"));
    }

    #[test]
    fn criar_tabela_vazia_desfaz_cabecalhos_na_mesma_transacao() {
        let mut app=App::default();
        super::super::criar(&mut app.workbook,0,Rect::new(0,0,3,1),
            "Tabela1",true,true).unwrap();
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(0,0),"Coluna1");
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(0,1),"Coluna2");
        assert!(app.workbook.nomeados().iter().any(|n|n.nome=="Tabela1"));
        let _=app.update(Msg::Desfazer);
        assert!(super::super::sob_celula(&app.workbook,0,0,0).is_none());
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(0,0),"");
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(0,1),"");
        assert!(!app.workbook.nomeados().iter().any(|n|n.nome=="Tabela1"));
        let _=app.update(Msg::Refazer);
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(0,0),"Coluna1");
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(0,1),"Coluna2");
        assert!(app.workbook.nomeados().iter().any(|n|n.nome=="Tabela1"));
    }

    #[test]
    fn converter_e_redimensionar_sem_filtros_respeitam_ctrl_z_ctrl_y() {
        let mut app=App::default();
        app.workbook.escrever_celula(0,0,0,"Produto".into());
        app.workbook.escrever_celula(0,1,0,"Caneta".into());
        super::super::criar(&mut app.workbook,0,Rect::new(0,0,1,0),
            "Tabela1",true,false).unwrap();
        let _=app.acao_tabela(Acao::Abrir);
        let _=app.acao_tabela(Acao::IniciarRedimensionamento);
        let _=app.acao_tabela(Acao::Intervalo("A1:B5".into()));
        let _=app.acao_tabela(Acao::Redimensionar);
        assert_eq!(super::sob_id(&app.workbook,0,"tab_Tabela1").unwrap()["area"]["r1"],4);
        assert_eq!(app.workbook.nomeados().iter().find(|n|n.nome=="Tabela1").unwrap().area.r1,4);
        let _=app.update(Msg::Desfazer);
        assert_eq!(super::sob_id(&app.workbook,0,"tab_Tabela1").unwrap()["area"]["r1"],1);
        assert_eq!(app.workbook.nomeados().iter().find(|n|n.nome=="Tabela1").unwrap().area.r1,1);
        let _=app.update(Msg::Refazer);
        assert_eq!(super::sob_id(&app.workbook,0,"tab_Tabela1").unwrap()["area"]["c1"],1);
        assert_eq!(app.workbook.nomeados().iter().find(|n|n.nome=="Tabela1").unwrap().area.c1,1);
        let _=app.acao_tabela(Acao::Converter);
        assert!(super::super::sob_celula(&app.workbook,0,0,0).is_none());
        assert!(!app.workbook.nomeados().iter().any(|n|n.nome=="Tabela1"));
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(1,0),"Caneta");
        let _=app.update(Msg::Desfazer);
        assert!(super::super::sob_celula(&app.workbook,0,0,0).is_some());
        assert_eq!(app.workbook.nomeados().iter().find(|n|n.nome=="Tabela1").unwrap().area.r1,4);
        let _=app.update(Msg::Refazer);
        assert!(super::super::sob_celula(&app.workbook,0,0,0).is_none());
        assert!(!app.workbook.nomeados().iter().any(|n|n.nome=="Tabela1"));
    }

    #[test]
    fn historico_de_tabela_nao_apaga_nomes_definidos_independentes() {
        let mut app=App::default();
        let nome_aba=app.workbook.aba(0).unwrap().sheet.name.clone();
        let manual=Rect::na_aba(40,2,50,2,nome_aba);
        app.workbook.definir_nome("MetaExterna",manual.clone());
        app.workbook.escrever_celula(0,0,0,"Item".into());
        app.workbook.escrever_celula(0,1,0,"A".into());
        super::super::criar(&mut app.workbook,0,Rect::new(0,0,1,0),
            "Tabela1",true,false).unwrap();
        let _=app.acao_tabela(Acao::Abrir);
        let _=app.acao_tabela(Acao::Nome("Estoque".into()));
        let _=app.acao_tabela(Acao::Renomear);
        let _=app.update(Msg::Desfazer);
        assert!(app.workbook.nomeados().iter().any(|n|n.nome=="Tabela1"));
        assert!(!app.workbook.nomeados().iter().any(|n|n.nome=="Estoque"));
        assert_eq!(app.workbook.nomeados().iter()
            .find(|n|n.nome=="MetaExterna").unwrap().area,manual);
        let _=app.update(Msg::Refazer);
        assert_eq!(app.workbook.nomeados().iter()
            .find(|n|n.nome=="MetaExterna").unwrap().area,manual);
    }

    #[test]
    fn painel_de_tabela_iced_013_suporta_texto_dinamico_e_filtro_decimal() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 0, 0, "Valor".into());
        app.workbook.escrever_celula(0, 1, 0, "2".into());
        super::super::criar(
            &mut app.workbook, 0, Rect::new(0, 0, 1, 0),
            "Tabela1", true, false,
        ).unwrap();
        app.tabela_editando = Some("tab_Tabela1".into());
        app.tabela_col_filtro = 0;
        // A construção exige Element<'_, Msg>, não String diretamente
        // como argumento de button() no Iced 0.13.
        {
            let _painel: Element<'_, Msg> = app.menu_filtro_tabela();
        }
        let _ = app.acao_tabela(Acao::Operador("gt"));
        let _ = app.acao_tabela(Acao::Criterio("1,5".into()));
        let _ = app.acao_tabela(Acao::AplicarFiltro);
        assert!(app.status.contains("Filtro aplicado"), "{}", app.status);
        let t = super::sob_id(&app.workbook, 0, "tab_Tabela1").unwrap();
        assert_eq!(t["filtros"][0]["criterio"]["val"], 1.5);
    }
    #[test]
    fn filtro_em_lotes_acha_valor_apos_mais_de_16_mil_linhas() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 0, 0, "Produto".into());
        app.workbook.escrever_celula(0, 1, 0, "A".into());
        app.workbook.escrever_celula(0, 17_000, 0, "FIM".into());
        super::super::criar(&mut app.workbook, 0, Rect::new(0, 0, 17_000, 0),
            "Tabela1", true, false).unwrap();
        app.tabela_editando = Some("tab_Tabela1".into());
        app.tabela_col_filtro = 0;
        app.carregar_opcoes_tabela();
        assert!(app.tabela_filtro_truncado);
        assert!(!app.tabela_filtro_vistos.contains("FIM"));
        let _ = app.acao_tabela(Acao::CarregarMais);
        assert!(!app.tabela_filtro_vistos.contains("FIM"));
        let _ = app.acao_tabela(Acao::CarregarMais);
        assert!(app.tabela_filtro_vistos.contains("FIM"));
        assert!(!app.tabela_filtro_truncado);
        let _ = app.acao_tabela(Acao::BuscaLista("fim".into()));
        assert_eq!(app.tabela_filtro_indices.len(), 1);
        assert_eq!(app.tabela_filtro_lista[app.tabela_filtro_indices[0]], "FIM");
    }

    #[test]
    fn filtro_parcial_nao_esconde_valores_ainda_nao_examinados() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 0, 0, "Produto".into());
        app.workbook.escrever_celula(0, 1, 0, "Remover".into());
        app.workbook.escrever_celula(0, 9_000, 0, "Preservar".into());
        super::super::criar(&mut app.workbook, 0, Rect::new(0, 0, 9_000, 0),
            "Tabela1", true, false).unwrap();
        app.tabela_editando = Some("tab_Tabela1".into());
        app.tabela_col_filtro = 0;
        app.carregar_opcoes_tabela();
        assert!(app.tabela_filtro_truncado);
        let _ = app.acao_tabela(Acao::MarcarValor("Remover".into()));
        let _ = app.acao_tabela(Acao::AplicarLista);
        let filtro = super::sob_id(&app.workbook, 0, "tab_Tabela1").unwrap();
        assert_eq!(filtro["filtros"][0]["criterio"]["op"], "not_in");
        assert_eq!(app.workbook.aba(0).unwrap().sheet.row_heights.get(&1), Some(&0));
        assert_ne!(app.workbook.aba(0).unwrap().sheet.row_heights.get(&9_000), Some(&0));
    }

    #[test]
    fn confirmar_amostra_sem_edicao_preserva_criterio_anterior() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 0, 0, "Produto".into());
        app.workbook.escrever_celula(0, 1, 0, "A".into());
        app.workbook.escrever_celula(0, 9_000, 0, "B".into());
        super::super::criar(&mut app.workbook, 0, Rect::new(0, 0, 9_000, 0),
            "Tabela1", true, false).unwrap();
        super::filtrar(&mut app.workbook, 0, "tab_Tabela1", 0,
            Some(json!(["B"]))).unwrap();
        let antes = super::sob_id(&app.workbook, 0, "tab_Tabela1").unwrap()["filtros"].clone();
        app.tabela_editando = Some("tab_Tabela1".into());
        app.tabela_col_filtro = 0;
        app.carregar_opcoes_tabela();
        assert!(app.tabela_filtro_truncado);
        let _ = app.acao_tabela(Acao::AplicarLista);
        let depois = super::sob_id(&app.workbook, 0, "tab_Tabela1").unwrap()["filtros"].clone();
        assert_eq!(antes, depois);
    }

}
