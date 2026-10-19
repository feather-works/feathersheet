//! Intervalos editáveis e sugestões de configuração de tabela dinâmica.
use super::*;
use feathersheet_core::dinamica::campos;

fn intervalo(texto: &str) -> Option<Rect> {
    feathersheet_core::celulas::refs::parse_range_ref(texto).map(|r| r.normalized())
}

pub(super) fn texto_valido(texto: &str) -> bool {
    intervalo(texto).is_some_and(|r| r.r0 < r.r1)
}

impl App {
    /// OK do primeiro passo: usa o intervalo digitado, se mudou.
    pub(super) fn aplicar_texto_antes_de_criar(&mut self) -> bool {
        let Some(e) = self.dinamica.as_ref() else { return false };
        match intervalo(&e.origem_texto) {
            None => {
                self.status = tr!("Intervalo inválido. Use, por exemplo, A1:D100.").into();
                false
            }
            Some(nova) if nova != e.origem => {
                self.aplicar_nova_origem_dinamica();
                self.dinamica.as_ref().is_some_and(|e| e.origem == nova)
            }
            Some(_) => true,
        }
    }

    pub(super) fn aplicar_nova_origem_dinamica(&mut self) {
        let Some(e) = self.dinamica.as_ref() else { return };
        let Some(nova) = intervalo(&e.origem_texto) else {
            self.status = tr!("Intervalo inválido. Use, por exemplo, A1:D100.").into();
            return;
        };
        if nova.r0 >= nova.r1 {
            self.status = tr!("A origem deve ter cabeçalho e pelo menos uma linha de dados.").into();
            return;
        }
        if nova == e.origem {
            self.status = tr!("A origem já corresponde ao intervalo informado.").into();
            return;
        }
        let folha = e.folha_origem;
        let campos = campos::campos(nova.clone(), |r,c| self.workbook.valor(folha,r,c));
        let col_medida = campos.iter().enumerate().rev().find(|(_,c)| c.numerico)
            .map(|(i,_)| nova.c0 as usize + i).unwrap_or(nova.c1 as usize);
        if let Some(e) = self.dinamica.as_mut() {
            e.origem = nova.clone();
            e.origem_texto = endereco_area(&nova);
            e.nomes = campos.into_iter().map(|c| c.nome).collect();
            e.linhas = vec![nova.c0 as usize];
            e.colunas.clear();
            e.valores = vec![MedidaDinamica{coluna:col_medida,op:AgregacaoOp::Soma}];
            e.calculados.clear();
            e.filtros.clear();
            e.recolhidos.clear();
            e.grupos.clear();
            e.campo_filtro = None;
            e.valores_filtro.clear();
            e.filtro_cortado = false;
        }
        self.status = tr!("Intervalo aplicado. Campos e filtros anteriores foram redefinidos.").into();
    }

    pub(super) fn preset_dinamica(&mut self, id: u8) {
        let Some(e) = self.dinamica.as_mut() else { return };
        let col0 = e.origem.c0 as usize;
        let largura = e.origem.c1 as usize - col0 + 1;
        if largura < 2 {
            self.status = tr!("É necessário ter pelo menos dois campos para as sugestões.").into();
            return;
        }
        let medida = e.valores.first().map(|v| v.coluna).unwrap_or(e.origem.c1 as usize);
        e.linhas = vec![col0];
        e.colunas.clear();
        e.valores = vec![MedidaDinamica {
            coluna: medida,
            op: if id == 1 { AgregacaoOp::Media } else { AgregacaoOp::Soma },
        }];
        if id == 2 {
            let candidato = (col0+1..=e.origem.c1 as usize)
                .find(|&col| col != medida)
                .unwrap_or(col0+1);
            e.colunas = vec![candidato];
        }
        self.status = tr!("Sugestão selecionada. Revise os campos e confirme a dinâmica.").into();
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    #[test]
    fn leitura_de_intervalo_com_coordenadas_absolutas() {
        assert_eq!(intervalo("B2:D20"),Some(Rect::new(1,1,19,3)));
        assert_eq!(intervalo("D20:B2"),Some(Rect::new(1,1,19,3)));
        assert!(intervalo("B0:D1").is_none());
        assert!(intervalo("texto").is_none());
    }

    #[test]
    fn troca_de_origem_redefine_campos_sem_tocar_nas_celulas() {
        let mut app = App::default();
        app.workbook.editar(0,|sheet|{
            sheet.set_cell(0,0,"A");sheet.set_cell(0,1,"B");
            sheet.set_cell(1,0,"x");sheet.set_cell(1,1,"12");
            sheet.set_cell(2,0,"y");sheet.set_cell(2,1,"15");
        });
        app.fim_selecao = (1,1);
        app.evento_dinamica(Evento::Abrir);
        crate::app::dinamica_ui::campos_de_antes(&mut app);
        app.evento_dinamica(Evento::OrigemTexto("A1:B3".into()));
        app.evento_dinamica(Evento::OrigemAplicar);
        assert_eq!(app.dinamica.as_ref().unwrap().origem.r1,2);
        assert_eq!(app.workbook.valor(0,2,1),Value::Number(15.));
    }
}
