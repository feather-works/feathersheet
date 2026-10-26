//! Aviso depois de exportar .xlsx: a planilha usa funções que o Excel não
//! tem (as do FeatherSheet e as criadas por macro). No Excel essas células
//! mostram #NOME?; o arquivo é gravado do mesmo jeito.
use super::*;
use pelicano::column;

impl App {
    /// Guarda o aviso se alguma função usada não existe no Excel.
    pub(super) fn avisar_funcoes_fora_do_excel(&mut self, usadas: &std::collections::BTreeSet<String>) {
        let fora: Vec<String> = usadas.iter()
            .filter(|n| feathersheet_core::funcoes::FORA_DO_EXCEL.contains(&n.as_str())
                || self.workbook.funcoes_externas.contains(n.as_str()))
            .map(|n| crate::idioma::nome_funcao(n))
            .collect();
        if fora.is_empty() { return; }
        self.monitor.registrar(Nivel::Aviso, tr!("Exportar"), &trf!(
            "O .xlsx usa funções que o Excel não tem: {}", fora.join(", ")));
        self.aviso_xlsx = Some(fora);
    }

    pub(super) fn sobrepor_aviso_xlsx<'a>(&'a self, base: Element<'a, Msg>) -> Element<'a, Msg> {
        use super::dialogo_interno::dlg;
        let Some(funcoes) = &self.aviso_xlsx else { return base };
        let mut lista = column(Vec::<pelicano::Element<_>>::new()).spacing(6);
        for nome in funcoes {
            lista = lista.push(row![
                feather_ui::icones::icone_traco("math-function", 15.),
                text(nome.clone()).size(13).font(pelicano::Font::MONO),
            ].spacing(8).align(pelicano::Align::Center));
        }
        let corpo = column![
            text(tr!("O arquivo foi exportado, mas usa funções que o Excel não tem. Ao abrir no Excel, as células com elas mostram #NOME? quando forem recalculadas."))
                .size(12.5).line_height(1.4),
            dlg::secao(tr!("Funções sem equivalente no Excel")),
            container(lista).padding([8, 12]).width(Length::FILL).style(dlg::estilo_bloco),
            text(tr!("No FeatherSheet (.fshz) elas continuam funcionando. Para usar o arquivo no Excel, troque essas fórmulas por valores (Colar especial, opção Valores) ou por funções do Excel."))
                .size(12).line_height(1.4).color(COR_TEXTO_MUTED()),
        ].spacing(12);
        self.dialogo(tr!("Funções que o Excel não tem"), corpo)
            .icone("alert-triangle")
            .subtitulo(tr!("Exportar para .xlsx"))
            .largura(520.0)
            .botao(dlg::botao_primario(tr!("Entendi"), Some(Msg::AvisoXlsxFechar)))
            .ao_fechar(Msg::AvisoXlsxFechar)
            .sobre(base)
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn aviso_lista_so_funcoes_fora_do_excel_no_idioma_da_interface() {
        let mut app = App::default();
        let usadas: std::collections::BTreeSet<String> = ["SUM", "COUNTUNIQUE", "MINHAFUNCAO"].map(String::from).into();
        app.avisar_funcoes_fora_do_excel(&usadas);
        assert_eq!(app.aviso_xlsx.as_ref().map(Vec::len), Some(1));
        // Função criada por macro também não existe no Excel.
        app.workbook.funcoes_externas.insert("MINHAFUNCAO".into());
        app.avisar_funcoes_fora_do_excel(&usadas);
        let lista = app.aviso_xlsx.clone().unwrap();
        assert_eq!(lista.len(), 2);
        assert!(!lista.contains(&"SUM".to_string()));
        let _: Element<'_, Msg> = app.sobrepor_aviso_xlsx(Space::new(0.0, 0.0).into());
        let _ = app.update(Msg::AvisoXlsxFechar);
        assert!(app.aviso_xlsx.is_none());
        // Só funções do Excel: nenhum aviso.
        app.avisar_funcoes_fora_do_excel(&["SUM".to_string()].into());
        assert!(app.aviso_xlsx.is_none());
    }
}
