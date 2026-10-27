//! Porta a mesclagem de `src/macros/api-navegacao.ts` para o host Rust.
//! Reutiliza `Rect`, `Sheet::alterar_mesclas` e snapshots de célula.
//! A macro legada LIMPA células subordinadas; o botão visual Iced preserva
//! conteúdos ocultos. São semânticas diferentes por decisão explícita.
use feathersheet_core::{
    celulas::refs::{key_row,key_col,Rect},
    operacoes::mesclagem::cruzam,
    planilha::sheet::Sheet,
};

/// Mantém a normalização própria do macro legado (sem nome de aba),
/// mas reusa o mesmo saneamento geométrico que a GUI Rust.
pub(crate) fn ler(sheet:&Sheet)->Vec<Rect> {
    let registros: Vec<Rect>=sheet.mc.as_deref()
        .and_then(|txt|serde_json::from_str(txt).ok()).unwrap_or_default();
    let registros=registros.into_iter().map(|mut r|{ r.sheet=None; r }).collect();
    feathersheet_core::operacoes::mesclagem::normalizar(registros)
}

/// Assim como `mesclaEm` do TS: verifica a PRIMEIRA célula do intervalo,
/// não qualquer sobreposição com ele.
pub(crate) fn esta_mesclada(sheet:&Sheet,area:&Rect)->bool {
    ler(sheet).iter().any(|m|m.contains(area.r0,area.c0))
}

/// Preparação pura: nenhuma mutação se o endereço ou a serialização falhar.
pub(crate) struct Plano {
    novo: Option<String>,
    limpar: Vec<(u32,u32)>,
    altera_meta: bool,
}
impl Plano {
    pub(crate) fn tem_alteracoes(&self)->bool {
        self.altera_meta || !self.limpar.is_empty()
    }
}

/// Macro `merge`: remove mesclas anteriores que cruzam o novo retângulo,
/// preserva a âncora e LIMPA o conteúdo das demais células da área. O limite
/// de 500k é conferido no host. A planilha pode ter 10M linhas: a enumeração
/// só passa por chaves presentes, sem materializar as posições vazias.
pub(crate) fn preparar(sheet:&Sheet,area:&Rect,mesclar:bool)->Result<Plano,String>{
    let area=area.normalized();
    if mesclar && area.r0==area.r1 && area.c0==area.c1 {
        return Err("merge: escolha mais de uma célula".into());
    }
    let mut lista=ler(sheet);
    let n=lista.len();
    lista.retain(|m|!cruzam(m,&area));
    if mesclar { lista.push(area.clone()); }
    if !mesclar && lista.len()==n {
        return Ok(Plano{
            novo:sheet.mc.clone(), limpar:Vec::new(), altera_meta:false,
        });
    }
    let novo=if lista.is_empty() { None }
        else { Some(serde_json::to_string(&lista)
            .map_err(|e|format!("Falha ao serializar mesclagens: {e}"))?) };
    let mut limpar=Vec::new();
    if mesclar {
        for chave in sheet.chaves_no_retangulo(&area) {
            let (r,c)=(key_row(chave),key_col(chave));
            if r==area.r0 && c==area.c0 { continue; }
            if sheet.snapshot(r,c).is_some_and(|snap|!snap.raw.is_empty()) {
                limpar.push((r,c));
            }
        }
    }
    let altera_meta=sheet.mc!=novo;
    Ok(Plano{novo,limpar,altera_meta})
}

/// Chamada com transação EXTERNA aberta pela macro. `alterar_mesclas`
/// usa transação aninhada; `apply` preserva estilo direto e fórmula no undo.
/// Isso permite um único Ctrl+Z e rollback de erro JavaScript.
pub(crate) fn aplicar(sheet:&mut Sheet,area:&Rect,plano:Plano){
    for (row,col) in plano.limpar {
        let Some(mut antigo)=sheet.snapshot(row,col) else { continue };
        antigo.raw.clear();
        let proximo=if antigo.style.is_some(){Some(antigo)}else{None};
        sheet.apply(row,col,proximo,true);
    }
    if plano.altera_meta {
        sheet.alterar_mesclas(plano.novo,area);
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn macro_mescla_apaga_subordinadas_mas_retem_estilo_e_undo() {
        let mut s=Sheet::new("Dados");
        s.set_cell(0,0,"Ancora");
        s.set_cell(0,1,"Apagar");
        s.set_cell(1,1,"=1+3");
        let mut estilo=feathersheet_core::celulas::estilo_parcial::EstiloParcial::new();
        estilo.insert("bold".into(),
            feathersheet_core::celulas::estilo_parcial::Campo::Logico(true));
        s.set_style(&Rect::cell(0,1),&estilo);
        let area=Rect::new(0,0,1,2);
        let plano=preparar(&s,&area,true).unwrap();
        assert!(plano.tem_alteracoes());
        s.abrir_transacao();
        aplicar(&mut s,&area,plano);
        s.fechar_transacao();
        assert!(esta_mesclada(&s,&Rect::cell(1,2)));
        assert_eq!(s.get_raw(0,0),"Ancora");
        assert_eq!(s.get_raw(0,1),"");
        assert_eq!(s.get_raw(1,1),"");
        assert!(s.estilo_mesclado(0,1).is_some());
        assert!(s.undo());
        assert_eq!(s.get_raw(0,1),"Apagar");
        assert_eq!(s.get_raw(1,1),"=1+3");
        assert!(!esta_mesclada(&s,&area));
    }

    #[test]
    fn unmerge_de_celula_interna_remove_bloco_inteiro() {
        let mut s=Sheet::new("Dados");
        let a=Rect::new(1,1,2,3);
        let plano=preparar(&s,&a,true).unwrap();
        s.abrir_transacao();
        aplicar(&mut s,&a,plano);
        s.fechar_transacao();
        let seletor=Rect::cell(2,2);
        let plano=preparar(&s,&seletor,false).unwrap();
        assert!(plano.tem_alteracoes());
        s.abrir_transacao();
        aplicar(&mut s,&seletor,plano);
        s.fechar_transacao();
        assert!(!esta_mesclada(&s,&a));
        assert!(s.undo());
        assert!(esta_mesclada(&s,&a));
    }

    #[test]
    fn mescla_invalida_e_desmescla_sem_regiao_nao_modificam() {
        let s=Sheet::new("Dados");
        assert!(preparar(&s,&Rect::cell(2,2),true).is_err());
        assert!(!esta_mesclada(&s,&Rect::cell(2,2)));
        assert!(!preparar(&s,&Rect::new(0,0,9,9),false)
            .unwrap().tem_alteracoes());
    }

    #[test]
    fn regionais_cruzadas_seguem_substituicao_da_lista_ts() {
        let mut s=Sheet::new("Dados");
        let a=Rect::new(0,0,1,1);
        let b=Rect::new(1,1,2,3);
        let plano=preparar(&s,&a,true).unwrap();
        s.abrir_transacao();
        aplicar(&mut s,&a,plano);
        s.fechar_transacao();
        let plano=preparar(&s,&b,true).unwrap();
        s.abrir_transacao();
        aplicar(&mut s,&b,plano);
        s.fechar_transacao();
        assert_eq!(ler(&s),vec![b]);
    }
}
