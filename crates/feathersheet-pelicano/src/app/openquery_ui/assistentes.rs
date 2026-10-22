//! Assistentes nativos OpenQuery: Junção, Acrescentar e Pivotar.
//! Os assistentes produzem exclusivamente Passo do motor Rust existente.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tipo { Juntar, Acrescentar, Pivotar }

#[derive(Clone, Debug)]
pub enum Evento {
    Abrir(Tipo), Fechar,
    Fonte(String), Modo(&'static str),
    ChaveEsquerda(usize), ChaveDireita(usize),
    PivotLinha(usize), PivotColuna(usize), PivotValor(usize),
    Agregacao(&'static str), Confirmar,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct OpcaoColuna {
    indice: usize,
    rotulo: String,
}
impl std::fmt::Display for OpcaoColuna {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.rotulo)
    }
}

pub(super) struct Estado {
    tipo: Tipo,
    fonte: Option<String>,
    esquerda: Vec<usize>,
    direita: Vec<usize>,
    modo: &'static str,
    linha: usize,
    coluna: usize,
    valor: usize,
    agregacao: &'static str,
}

impl Estado {
    fn novo(tipo: Tipo, editor: &super::Estado) -> Self {
        Self {
            tipo,
            fonte: editor.fontes.keys().min().cloned(),
            esquerda: vec![editor.coluna.min(editor.resultado.cabecalho.len().saturating_sub(1))],
            direita: vec![0],
            modo: "esquerda",
            linha: 0, coluna: 1, valor: 2,
            agregacao: "soma",
        }
    }
}

fn alternar_chave(lista: &mut Vec<usize>, indice: usize) {
    if let Some(i) = lista.iter().position(|col| *col == indice) {
        lista.remove(i);
    } else {
        lista.push(indice);
    }
}
fn passo(editor: &super::Estado, e: &Estado) -> Result<Passo, String> {
    match e.tipo {
        Tipo::Acrescentar => {
            let fonte = e.fonte.clone().ok_or(tr!("Selecione a segunda tabela para acrescentar"))?;
            if !editor.fontes.contains_key(&fonte) {
                return Err(tr!("A fonte escolhida não está carregada").into());
            }
            Ok(Passo::Acrescentar { fonte })
        }
        Tipo::Juntar => {
            let fonte = e.fonte.clone().ok_or(tr!("Selecione a segunda tabela para mesclar"))?;
            let secundaria = editor.fontes.get(&fonte).ok_or(tr!("Fonte secundária indisponível"))?;
            if e.esquerda.is_empty() || e.direita.is_empty() ||
                e.esquerda.len() != e.direita.len() {
                return Err(tr!("Selecione a mesma quantidade de colunas-chave nas duas tabelas").into());
            }
            if e.esquerda.iter().any(|c| *c >= editor.resultado.cabecalho.len()) ||
                e.direita.iter().any(|c| *c >= secundaria.cabecalho.len()) {
                return Err(tr!("Uma coluna-chave não existe mais; refaça a seleção").into());
            }
            let esquerda = if e.esquerda.len() == 1 {
                IndiceColuna::Single(e.esquerda[0])
            } else {
                IndiceColuna::Multiple(e.esquerda.clone())
            };
            let direita = if e.direita.len() == 1 {
                IndiceColuna::Single(e.direita[0])
            } else {
                IndiceColuna::Multiple(e.direita.clone())
            };
            Ok(Passo::Juntar {
                fonte, esquerda, direita, modo: Some(e.modo.into()),
            })
        }
        Tipo::Pivotar => {
            let total = editor.resultado.cabecalho.len();
            if total < 3 {
                return Err(tr!("Pivotar exige ao menos três colunas: linha, categoria e valor").into());
            }
            if [e.linha, e.coluna, e.valor].iter().any(|c| *c >= total) ||
                e.linha == e.coluna || e.linha == e.valor || e.coluna == e.valor {
                return Err(tr!("Linha, categoria e valor precisam ser colunas diferentes").into());
            }
            Ok(Passo::Pivotar {
                linhas: vec![e.linha], colunas: vec![e.coluna],
                valores: vec![Agregado {
                    col: e.valor,
                    como: Agregacao::do_str(e.agregacao),
                    nome: Some(editor.resultado.cabecalho[e.valor].clone()),
                }],
            })
        }
    }
}

impl App {
    pub(super) fn evento_assistente_openquery(&mut self, evento: Evento) {
        let Some(editor) = self.openquery.as_mut() else { return };
        match evento {
            Evento::Abrir(tipo) => {
                if let Err(erro) = editor.sincronizar() {
                    editor.erro = erro;
                    return;
                }
                editor.assistente = Some(Estado::novo(tipo, editor));
                editor.erro.clear();
                return;
            }
            Evento::Fechar => { editor.assistente = None; return; }
            _ => {}
        }
        if let Evento::Confirmar = evento {
            let Some(w) = editor.assistente.take() else { return };
            match passo(editor, &w) {
                Ok(passo) => {
                    editor.passos.push(passo);
                    editor.registrar_passos();
                }
                Err(erro) => {
                    editor.assistente = Some(w);
                    editor.erro = erro;
                }
            }
            return;
        }
        let Some(e) = editor.assistente.as_mut() else { return };
        match evento {
            Evento::Fonte(nome) => {
                if editor.fontes.contains_key(&nome) {
                    e.fonte = Some(nome);
                    e.direita = vec![0];
                }
            }
            Evento::Modo(modo) => e.modo = modo,
            Evento::ChaveEsquerda(i) => alternar_chave(&mut e.esquerda, i),
            Evento::ChaveDireita(i) => alternar_chave(&mut e.direita, i),
            Evento::PivotLinha(i) => e.linha = i,
            Evento::PivotColuna(i) => e.coluna = i,
            Evento::PivotValor(i) => e.valor = i,
            Evento::Agregacao(a) => e.agregacao = a,
            Evento::Abrir(_) | Evento::Fechar | Evento::Confirmar => {}
        }
    }

    fn coluna_opcoes<'a>(
        nomes: &'a [String], selecionadas: &'a [usize],
        evento: fn(usize) -> Evento,
    ) -> Element<'a, Msg> {
        let mut itens = pelicano::column(Vec::<Element<Msg>>::new()).spacing(ESPACO_PEQUENO);
        for (i, nome) in nomes.iter().enumerate().take(64) {
            // Chaves marcadas mostram a ordem da seleção; as demais, só o nome.
            let rotulo = match selecionadas.iter().position(|c| *c == i) {
                Some(ordem) => format!("{}. {} · {}", ordem + 1, i + 1, nome),
                None => format!("{} · {}", i + 1, nome),
            };
            itens = itens.push(button(text(rotulo).size(TAM_TEXTO))
                .on_press(Msg::OpenQuery(super::Evento::Assistente(evento(i))))
                .style(estilo_botao_pequeno(selecionadas.contains(&i)))
                .width(Length::FILL).padding([4.0, 6.0]));
        }
        scrollable(itens).height(Length::Fixed(140.0)).into()
    }

    pub(super) fn painel_assistente_openquery<'a>(&'a self, editor: &'a super::Estado)
        -> Element<'a, Msg>
    {
        let Some(w) = &editor.assistente else { return Space::new(Length::Fixed(0.0), Length::Fixed(0.0)).into() };
        let titulo = match w.tipo {
            Tipo::Juntar => tr!("Mesclar consultas"),
            Tipo::Acrescentar => tr!("Acrescentar consultas"),
            Tipo::Pivotar => tr!("Pivotar dados"),
        };
        let mut corpo = pelicano::column![
            titulo_oq(titulo),
            legenda_oq(tr!("As escolhas viram uma etapa editável da consulta.")),
        ].spacing(ESPACO_MEDIO);
        if w.tipo != Tipo::Pivotar {
            let mut fontes = pelicano::row(Vec::<Element<Msg>>::new()).spacing(ESPACO_PEQUENO);
            for nome in editor.fontes.keys().take(10) {
                let marcada = w.fonte.as_ref() == Some(nome);
                fontes = fontes.push(button(text(nome.clone()).size(TAM_TEXTO))
                    .on_press(Msg::OpenQuery(super::Evento::Assistente(
                        Evento::Fonte(nome.clone()),
                    )))
                    .style(estilo_botao_pequeno(marcada))
                    .padding([4.0, 8.0]));
            }
            corpo = corpo.push(secao_oq(tr!("Fonte secundária"))).push(fontes);
            if editor.fontes.is_empty() {
                corpo = corpo.push(legenda_oq(
                    tr!("Primeiro adicione outra aba em Fontes adicionais, na lateral esquerda.")
                ));
            }
        }
        if w.tipo == Tipo::Juntar {
            let mut modos = pelicano::column(Vec::<Element<Msg>>::new()).spacing(ESPACO_PEQUENO);
            for grupo in [
                [(tr!("Esquerda"), "esquerda"), (tr!("Interna"), "interna"), (tr!("Direita"), "direita")],
                [(tr!("Total"), "total"), (tr!("Anti esquerda"), "anti_esquerda"), (tr!("Anti direita"), "anti_direita")],
            ] {
                let mut linha = pelicano::row(Vec::<Element<Msg>>::new()).spacing(ESPACO_GRANDE);
                for (rotulo, codigo) in grupo {
                    linha = linha.push(pelicano::radio(rotulo, codigo, Some(w.modo), |v| {
                        Msg::OpenQuery(super::Evento::Assistente(Evento::Modo(v)))
                    }).size(TAM_TEXTO));
                }
                modos = modos.push(linha);
            }
            corpo = corpo.push(secao_oq(tr!("Tipo de junção"))).push(modos);
            let secundarias = w.fonte.as_ref()
                .and_then(|nome| editor.fontes.get(nome));
            let nomes_direita = secundarias.map(|t| t.cabecalho.as_slice()).unwrap_or(&[]);
            let colunas = pelicano::row![
                pelicano::column![
                    legenda_oq(tr!("Chaves da primeira tabela (ordem da seleção)")),
                    Self::coluna_opcoes(
                        &editor.resultado.cabecalho, &w.esquerda, Evento::ChaveEsquerda,
                    ),
                ].width(Length::Fill(1)).spacing(ESPACO_PEQUENO),
                pelicano::column![
                    legenda_oq(tr!("Chaves da segunda tabela (mesma ordem)")),
                    Self::coluna_opcoes(
                        nomes_direita, &w.direita, Evento::ChaveDireita,
                    ),
                ].width(Length::Fill(1)).spacing(ESPACO_PEQUENO),
            ].spacing(ESPACO_GRANDE);
            corpo = corpo.push(colunas);
        }
        if w.tipo == Tipo::Pivotar {
            for (rotulo, escolha, evento) in [
                (tr!("Linhas"), w.linha, Evento::PivotLinha as fn(usize) -> Evento),
                (tr!("Categorias (viram colunas)"), w.coluna, Evento::PivotColuna),
                (tr!("Valores"), w.valor, Evento::PivotValor),
            ] {
                let opcoes: Vec<String> = editor.resultado.cabecalho.iter()
                    .enumerate().map(|(i,nome)| OpcaoColuna {
                        indice: i, rotulo: format!("{} · {}", i+1, nome),
                    }.to_string()).collect();
                let selecionada = opcoes.get(escolha).cloned();
                corpo = corpo.push(pelicano::row![
                    container(texto_oq(rotulo)).width(Length::Fill(2)),
                    crate::compat::pick_list_vec(opcoes.clone(), selecionada, move |escolhida: String| {
                        let indice = opcoes.iter().position(|o| *o == escolhida).unwrap_or(0);
                        Msg::OpenQuery(super::Evento::Assistente(evento(indice)))
                    }).size(TAM_TEXTO).padding([4.0, 8.0])
                        .width(Length::Fill(4)),
                ].spacing(ESPACO_MEDIO).align(pelicano::Align::Center));
            }
            let mut opcoes = pelicano::row(Vec::<Element<Msg>>::new()).spacing(ESPACO_GRANDE);
            for (rotulo, code) in [
                (tr!("Soma"), "soma"), (tr!("Contagem"), "contagem"), (tr!("Média"), "media"),
                (tr!("Máximo"), "maximo"), (tr!("Mínimo"), "minimo"),
            ] {
                opcoes = opcoes.push(pelicano::radio(rotulo, code, Some(w.agregacao), |v| {
                    Msg::OpenQuery(super::Evento::Assistente(Evento::Agregacao(v)))
                }).size(TAM_TEXTO));
            }
            corpo = corpo.push(secao_oq(tr!("Agregação"))).push(opcoes);
        }
        corpo = corpo.push(pelicano::row![
            Space::new(Length::FILL, 0.0),
            button(text(tr!("Cancelar")).size(TAM_TEXTO))
                .on_press(Msg::OpenQuery(super::Evento::Assistente(Evento::Fechar)))
                .style(estilo_botao_secundario).padding([4.0, 8.0]),
            button(text(tr!("Aplicar etapa")).size(TAM_TEXTO))
                .on_press(Msg::OpenQuery(super::Evento::Assistente(Evento::Confirmar)))
                .style(estilo_botao_primario).padding([4.0, 8.0]),
        ].spacing(ESPACO_MEDIO));
        container(corpo.padding(PADDING_PAINEL)).width(Length::Fixed(730.0))
            .style(estilo_janela_oq).into()
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    fn editor() -> super::super::Estado {
        let fonte = Tabela {
            cabecalho: vec!["ID".into(), "Categoria".into(), "Valor".into()],
            linhas: vec![
                vec![Value::Number(1.),Value::Text("A".into()),Value::Number(5.)],
                vec![Value::Number(2.),Value::Text("B".into()),Value::Number(10.)],
            ],
        };
        let mut e = super::super::Estado::novo(fonte.clone(), "Planilha1".into(), Rect::new(0,0,2,2));
        e.fontes.insert("Segunda".into(), fonte);
        e
    }
    #[test]
    fn juncoes_suportam_seis_tipos_e_chaves_multiplas() {
        let e = editor();
        let mut w = Estado::novo(Tipo::Juntar, &e);
        w.esquerda = vec![0,1]; w.direita = vec![0,1];
        for modo in ["esquerda","interna","direita","total","anti_esquerda","anti_direita"] {
            w.modo = modo;
            let passo = passo(&e,&w).unwrap();
            assert!(matches!(passo,Passo::Juntar { .. }));
        }
        w.direita = vec![0];
        assert!(passo(&e,&w).is_err());
    }
    #[test]
    fn pivotar_exige_tres_colunas_diferentes() {
        let e = editor();
        let mut w = Estado::novo(Tipo::Pivotar, &e);
        assert!(matches!(passo(&e,&w),Ok(Passo::Pivotar { .. })));
        w.valor = w.linha;
        assert!(passo(&e,&w).is_err());
    }
}
