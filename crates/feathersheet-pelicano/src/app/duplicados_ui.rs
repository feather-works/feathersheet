//! Remover duplicados — paridade com ui/caixas/duplicados-dialog.ts.
//! O diálogo nunca altera dados antes da confirmação; compara apenas as
//! colunas marcadas e compacta a região inteira pelo motor já existente.
use super::*;
use feathersheet_core::{Value, col_name, operacoes::ops};
use std::collections::BTreeSet;
use pelicano::checkbox;

#[derive(Debug, Clone)]
pub enum Evento {
    Coluna(u32),
    Todas,
    Nenhuma,
    Cabecalho(bool),
    Confirmar,
    Cancelar,
}

/// Estado da janela, isolado da seleção que o usuário pode mudar depois.
pub(super) struct Estado {
    pub folha: usize,
    pub regiao: Rect,
    pub marcadas: BTreeSet<u32>,
    pub cabecalho: bool,
    pub titulos: Vec<String>,
    pub erro: String,
}
impl Estado {
    fn novo(app: &App) -> Option<Self> {
        let aba = app.workbook.aba(app.folha)?;
        if aba.sheet.is_empty() { return None; }
        let selecao = app.area_selecionada().normalized();
        let regiao = ops::regiao_atual(&aba.sheet, selecao.clone());
        let titulos: Vec<String> = (regiao.c0..=regiao.c1).map(|c| {
            match app.workbook.valor(app.folha, regiao.r0, c) {
                Value::Text(s) => s,
                Value::Number(n) => n.to_string(),
                Value::Bool(b) => b.to_string(),
                Value::Error(e) => e.to_string(),
                Value::Empty => String::new(),
            }
        }).collect();
        let cabecalho = regiao.r1>regiao.r0 && (regiao.c0..=regiao.c1).all(|c| {
            matches!(app.workbook.valor(app.folha, regiao.r0, c), Value::Text(ref s) if !s.is_empty())
        });
        let marcadas: BTreeSet<u32> = (selecao.c0.max(regiao.c0)
            ..=selecao.c1.min(regiao.c1)).collect();
        Some(Self {
            folha:app.folha, regiao, marcadas, cabecalho, titulos,
            erro:String::new(),
        })
    }
    fn rotulo(&self, coluna:u32)->String {
        let letra=col_name(coluna);
        let titulo=self.titulos.get((coluna-self.regiao.c0) as usize)
            .map(String::as_str).unwrap_or("");
        if self.cabecalho && !titulo.is_empty() {
            format!("{letra}   {titulo}")
        }else{trf!("Coluna {letra}")}
    }
}

impl App {
    pub(super) fn abrir_duplicados(&mut self)->Task<Msg> {
        self.confirmar();
        if self.janelas.aberta(Auxiliar::RemoverDuplicados) {return Task::none();}
        let Some(estado)=Estado::novo(self) else {
            self.status=tr!("Não há dados para remover duplicados na aba atual").into();
            return Task::none();
        };
        self.duplicados=Some(estado);
        self.alternar(Auxiliar::RemoverDuplicados)
    }

    pub(super) fn evento_duplicados(&mut self, evento:Evento)->Task<Msg> {
        if matches!(evento,Evento::Cancelar) {
            self.duplicados=None;
            return self.janelas.fechar_tipo(Auxiliar::RemoverDuplicados);
        }
        if matches!(evento,Evento::Confirmar) {
            let Some(estado)=self.duplicados.as_ref() else {return Task::none()};
            if estado.marcadas.is_empty(){
                if let Some(e)=self.duplicados.as_mut(){
                    e.erro=tr!("Selecione ao menos uma coluna para comparar.").into();
                }
                return Task::none();
            }
            let folha=estado.folha;
            let area=estado.regiao.clone();
            let colunas:Vec<u32>=estado.marcadas.iter().copied().collect();
            let cabecalho=estado.cabecalho;
            let max_row=match self.workbook.aba(folha) {
                Some(a)=>a.sheet.used_rect().r1,
                None=>return Task::none(),
            };
            let plano=ops::planejar_repetidas(area.clone(),cabecalho,Some(&colunas),
                max_row,|r,c|self.workbook.valor(folha,r,c));
            let removidas=if let Some(plano)=plano{
                let total=plano.removidas;
                self.workbook.editar(folha,|sheet|{
                    ops::aplicar_repetidas(sheet,area.c0,area.c1,&plano)
                });
                self.alterado=true;
                total
            }else{0};
            self.status=if removidas>0 {
                trf!("{removidas} linha(s) duplicada(s) removida(s)")
            }else{tr!("Nenhum registro duplicado encontrado").into()};
            if self.folha==folha {
                self.edicao=self.workbook.aba(folha)
                    .map(|a|a.sheet.get_raw(self.linha,self.coluna))
                    .unwrap_or_default();
            }
            self.duplicados=None;
            return self.janelas.fechar_tipo(Auxiliar::RemoverDuplicados);
        }
        let Some(e)=self.duplicados.as_mut() else{return Task::none()};
        e.erro.clear();
        match evento{
            Evento::Coluna(c)=>{
                if (e.regiao.c0..=e.regiao.c1).contains(&c){
                    if !e.marcadas.insert(c){e.marcadas.remove(&c);}
                }
            }
            Evento::Todas=>e.marcadas=(e.regiao.c0..=e.regiao.c1).collect(),
            Evento::Nenhuma=>e.marcadas.clear(),
            Evento::Cabecalho(b)=>e.cabecalho=b,
            Evento::Cancelar|Evento::Confirmar=>unreachable!(),
        }
        Task::none()
    }

    pub(super) fn view_duplicados(&self)->Element<'_,Msg> {
        let Some(e)=self.duplicados.as_ref() else{
            return container(text(tr!("Janela fechada"))).into();
        };
        let intervalo=endereco_area(&e.regiao);
        let mut opcoes=pelicano::column(Vec::<Element<_>>::new()).spacing(3);
        for col in e.regiao.c0..=e.regiao.c1 {
            opcoes=opcoes.push(
                container(
                    checkbox(e.rotulo(col),e.marcadas.contains(&col))
                        .on_toggle(move |_|Msg::Duplicados(Evento::Coluna(col)))
                        .size(13)
                ).padding([7,10]).width(Length::FILL)
            );
        }
        let erro:Element<'_,Msg>=if e.erro.is_empty(){
            Space::new(0.0, Length::Fixed(0.)).into()
        }else{text(e.erro.clone()).size(12).color(COR_AZUL_ACENTO()).into()};
        let botoes=pelicano::row![
            button(tr!("Cancelar")).style(estilo_botao_secundario).padding([9,18])
                .on_press(Msg::Duplicados(Evento::Cancelar)),
            Space::new(Length::FILL, 0.0),
            button(tr!("Remover duplicados")).style(estilo_botao_primario)
                .padding([9,17])
                .on_press_maybe((!e.marcadas.is_empty())
                    .then_some(Msg::Duplicados(Evento::Confirmar))),
        ].align(pelicano::Align::Center).spacing(10);
        let corpo=pelicano::column![
            text(trf!("Região: {intervalo}")).size(14).color(COR_TEXTO()),
            text(tr!("Escolha quais colunas determinam se uma linha é repetida. A linha inteira será movida para manter os dados alinhados."))
                .size(12).color(COR_TEXTO_MUTED()),
            pelicano::row![
                button(tr!("Marcar todas")).style(estilo_botao_pequeno(false))
                    .padding([6,9]).on_press(Msg::Duplicados(Evento::Todas)),
                button(tr!("Desmarcar todas")).style(estilo_botao_pequeno(false))
                    .padding([6,9]).on_press(Msg::Duplicados(Evento::Nenhuma)),
                Space::new(Length::FILL, 0.0),
                text(trf!("{} de {} colunas",e.marcadas.len(),
                    e.regiao.c1-e.regiao.c0+1)).size(11).color(COR_TEXTO_MUTED()),
            ].spacing(10).align(pelicano::Align::Center),
            container(
                scrollable(opcoes).height(Length::FILL)
            ).height(Length::FILL).width(Length::FILL).padding(6)
                .style(estilo_painel_faixa),
            checkbox(tr!("Meus dados têm cabeçalho"),e.cabecalho)
                .on_toggle(|b|Msg::Duplicados(Evento::Cabecalho(b)))
                .size(13),
            erro,
            botoes,
        ].spacing(12).padding(18).width(Length::FILL).height(Length::FILL);
        container(corpo).height(Length::FILL).width(Length::FILL)
            .style(estilo_container_topo).into()
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    #[test]
    fn clicar_na_ferramenta_abre_dialogo_sem_apagar_linhas() {
        let mut app=App::default();
        for (r,linha) in [["ID","Cidade","Obs"],["1","SP","a"],
            ["2","SP","b"],["3","RJ","c"]].iter().enumerate(){
            for (c,v) in linha.iter().enumerate(){
                app.workbook.escrever_celula(0,r as u32,c as u32,(*v).into());
            }
        }
        app.linha=1;app.coluna=1;app.fim_selecao=(1,1);
        let _=app.update(Msg::RemoverDuplicados);
        let e=app.duplicados.as_ref().unwrap();
        assert_eq!(e.regiao,Rect::new(0,0,3,2));
        assert!(e.cabecalho);
        assert_eq!(e.marcadas.iter().copied().collect::<Vec<_>>(),vec![1]);
        assert_eq!(app.workbook.valor(0,2,0),Value::Number(2.));
        let _:Element<'_,Msg>=app.view_duplicados();
    }
    #[test]
    fn colunas_escolhidas_removem_linha_inteira_e_desfazer_restaura() {
        let mut app=App::default();
        for (r,linha) in [["ID","Cidade","Obs"],["1","SP","a"],
            ["2","SP","b"],["3","RJ","c"]].iter().enumerate(){
            for (c,v) in linha.iter().enumerate(){
                app.workbook.escrever_celula(0,r as u32,c as u32,(*v).into());
            }
        }
        app.linha=1;app.coluna=1;app.fim_selecao=(1,1);
        let _=app.update(Msg::RemoverDuplicados);
        let _=app.update(Msg::Duplicados(Evento::Confirmar));
        assert_eq!(app.workbook.valor(0,2,0),Value::Number(3.));
        assert_eq!(app.workbook.valor(0,2,2),Value::Text("c".into()));
        assert_eq!(app.workbook.valor(0,3,0),Value::Empty);
        let _=app.update(Msg::Desfazer);
        assert_eq!(app.workbook.valor(0,2,0),Value::Number(2.));
        assert_eq!(app.workbook.valor(0,2,2),Value::Text("b".into()));
    }
    #[test]
    fn desmarcar_todas_impede_confirmar_e_cancelar_preserva_dados() {
        let mut app=App::default();
        app.workbook.escrever_celula(0,0,0,"a".into());
        app.workbook.escrever_celula(0,1,0,"a".into());
        let _=app.update(Msg::RemoverDuplicados);
        let _=app.update(Msg::Duplicados(Evento::Nenhuma));
        let _=app.update(Msg::Duplicados(Evento::Confirmar));
        assert!(app.duplicados.as_ref().unwrap().erro.contains("Selecione"));
        let _=app.update(Msg::Duplicados(Evento::Cancelar));
        assert!(app.duplicados.is_none());
        assert_eq!(app.workbook.valor(0,1,0),Value::Text("a".into()));
    }
}
