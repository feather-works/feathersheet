//! As funções `vba.*` que o código gerado usa: o que o VBA faz de um jeito que o
//! JavaScript não faz igual (comparar e somar Variant, `Mid` contando do 1, arredondar
//! para o par, `Format`, `MsgBox`). Cada macro leva só as que usa — e as que essas usam.

use std::collections::BTreeSet;

/// (nome, dependências, código do método no objeto `vba`).
const AJUDAS: &[(&str, &[&str], &str)] = &[
    ("vazio", &[], "  /** Empty do VBA: célula vazia, variável sem valor. */\n  vazio(v) { return v === null || v === undefined || v === ''; },"),
    ("str", &[], "  /** O valor como texto, como o VBA (Empty vira \"\", True vira \"True\"). */\n  str(v) { if (v === null || v === undefined) return ''; if (typeof v === 'boolean') return v ? 'True' : 'False'; return String(v); },"),
    ("numero", &[], r#"  /** Texto que é número (aceita vírgula ou ponto decimal); NaN se não for. Sem regex: no motor das macros ela é recompilada a cada chamada. */
  numero(v) { if (typeof v === 'number') return v; const t = String(v).trim(); const n = t.length; let i = 0, dig = 0, sep = -1; if (t[0] === '+' || t[0] === '-') i++; for (; i < n; i++) { const c = t.charCodeAt(i); if (c >= 48 && c <= 57) dig++; else if ((c === 44 || c === 46) && sep < 0) sep = i; else break; } if (!dig) return NaN; if (i < n) { if (t[i] !== 'e' && t[i] !== 'E') return NaN; let j = i + 1; if (t[j] === '+' || t[j] === '-') j++; const k = j; while (j < n && t.charCodeAt(j) >= 48 && t.charCodeAt(j) <= 57) j++; if (j === k || j < n) return NaN; } return Number(sep < 0 ? t : t.slice(0, sep) + '.' + t.slice(sep + 1)); },"#),
    ("n", &["vazio", "numero"], "  /** O valor como número, como o VBA: Empty é 0, True é -1; texto que não é número dá o erro 13. */\n  n(v) { if (vba.vazio(v)) return 0; if (typeof v === 'boolean') return v ? -1 : 0; const x = vba.numero(v); if (Number.isNaN(x)) throw new TypeError(`Tipo incompatível: \"${v}\" não é número`); return x; },"),
    ("cat", &["str"], "  /** O `&` do VBA: junta como texto. */\n  cat(...p) { return p.map(vba.str).join(''); },"),
    ("soma", &["n"], "  /** O `+` do VBA com Variant: dois textos se juntam, o resto soma. */\n  soma(a, b) { return typeof a === 'string' && typeof b === 'string' ? a + b : vba.n(a) + vba.n(b); },"),
    ("cmp", &["vazio", "numero", "str"], "  /** Compara dois Variant como o VBA: -1, 0 ou 1. Número contra texto numérico compara como número. */\n  cmp(a, b) { const lg = (x) => (typeof x === 'string' && /^(true|verdadeiro)$/i.test(x.trim()) ? true : typeof x === 'string' && /^(false|falso)$/i.test(x.trim()) ? false : x); if (typeof a === 'boolean' || typeof b === 'boolean') { a = lg(a); b = lg(b); } if (typeof a === 'boolean') a = a ? -1 : 0; if (typeof b === 'boolean') b = b ? -1 : 0; if (vba.vazio(a) && vba.vazio(b)) return 0; if (typeof a === 'number' || typeof b === 'number') { const x = vba.vazio(a) ? 0 : vba.numero(a); const y = vba.vazio(b) ? 0 : vba.numero(b); if (!Number.isNaN(x) && !Number.isNaN(y)) return x < y ? -1 : x > y ? 1 : 0; return typeof a === 'number' ? -1 : 1; } const s = vba.str(a); const t = vba.str(b); return s < t ? -1 : s > t ? 1 : 0; },"),
    ("eq", &["cmp"], "  /** O `=` do VBA entre dois Variant. */\n  eq(a, b) { if (typeof a === typeof b && (typeof a === 'string' || typeof a === 'number')) return a === b; return vba.cmp(a, b) === 0; },"),
    ("bool", &["vazio", "numero"], "  /** O valor como verdadeiro/falso, como o `If` do VBA. */\n  bool(v) { if (typeof v === 'boolean') return v; if (vba.vazio(v)) return false; if (typeof v === 'number') return v !== 0; const t = String(v).trim().toLowerCase(); if (t === 'true' || t === 'verdadeiro') return true; if (t === 'false' || t === 'falso') return false; const x = vba.numero(t); if (Number.isNaN(x)) throw new TypeError(`Tipo incompatível: \"${v}\" não é verdadeiro/falso`); return x !== 0; },"),
    ("len", &["str"], "  len(v) { return vba.str(v).length; },"),
    ("mid", &["str", "n"], "  /** Mid(texto, início, [quantos]) — começa do 1. */\n  mid(s, i, q) { const t = vba.str(s); const a = Math.max(0, vba.n(i) - 1); return q === undefined ? t.slice(a) : t.slice(a, a + Math.max(0, vba.n(q))); },"),
    ("left", &["str", "n"], "  left(s, q) { return vba.str(s).substring(0, Math.max(0, vba.n(q))); },"),
    ("right", &["str", "n"], "  right(s, q) { const t = vba.str(s); return t.substring(Math.max(0, t.length - vba.n(q))); },"),
    ("instr", &["str", "n"], "  /** InStr([início,] texto, procurado): a posição (do 1) ou 0. */\n  instr(a, b, c) { let ini = 1; if (c !== undefined) { ini = vba.n(a); a = b; b = c; } return vba.str(a).indexOf(vba.str(b), ini - 1) + 1; },"),
    ("instrRev", &["str"], "  instrRev(s, b) { return vba.str(s).lastIndexOf(vba.str(b)) + 1; },"),
    ("replace", &["str"], "  replace(s, de, para) { return vba.str(s).split(vba.str(de)).join(vba.str(para)); },"),
    ("trim", &["str"], r#"  /** Trim do VBA: só espaço, não tabulação. */
  trim(s) { s = vba.str(s); let a = 0, b = s.length; while (a < b && s.charCodeAt(a) === 32) a++; while (b > a && s.charCodeAt(b - 1) === 32) b--; return s.slice(a, b); },"#),
    ("ltrim", &["str"], "  ltrim(s) { return vba.str(s).replace(/^ +/, ''); },"),
    ("rtrim", &["str"], "  rtrim(s) { return vba.str(s).replace(/ +$/, ''); },"),
    ("isNumeric", &["numero"], "  isNumeric(v) { if (v === null || v === undefined || typeof v === 'number' || typeof v === 'boolean') return true; return !Number.isNaN(vba.numero(v)); },"),
    ("val", &["str"], "  /** Val: o número do começo do texto (ponto decimal, ignora espaços). */\n  val(s) { const m = vba.str(s).replace(/\\s+/g, '').match(/^[+-]?(\\d+\\.?\\d*|\\.\\d+)(e[+-]?\\d+)?/i); return m ? Number(m[0]) : 0; },"),
    ("round", &["n"], "  /** Arredonda para o par no empate (o Round/CInt do VBA): 2,5 → 2; 3,5 → 4. */\n  round(x, casas = 0) { const f = 10 ** vba.n(casas); const v = vba.n(x) * f; const r = Math.round(v); const par = Math.abs(v % 1) === 0.5 ? 2 * Math.round(v / 2) : r; return par / f; },"),
    ("cint", &["round"], "  cint(v) { return vba.round(v); },"),
    ("mod", &["cint"], "  /** Mod do VBA: arredonda os dois e fica com o sinal de quem divide. */\n  mod(a, b) { return vba.cint(a) % vba.cint(b); },"),
    ("divInt", &["cint"], "  /** A barra invertida do VBA: divisão inteira. */\n  divInt(a, b) { return Math.trunc(vba.cint(a) / vba.cint(b)); },"),
    ("format", &["str", "numero", "formatoData", "mascara"], r#"  /** Format(valor, máscara): números (0 # . , % e literais com \x ou "…") e datas (dd/mm/yyyy, hh:nn, mmmm). */
  format(v, m) {
    m = vba.str(m);
    const nomeados = { fixed: '0.00', standard: '#,##0.00', percent: '0.00%', currency: 'R$ #,##0.00' };
    if (nomeados[m.toLowerCase()]) m = nomeados[m.toLowerCase()];
    const guardadas = vba._mascaras ?? (vba._mascaras = {});
    const g = guardadas[m] ?? (guardadas[m] = ((f) => ({ fichas: f, data: !f.some((x) => x.d) && /[dmyhns]/i.test(f.map((x) => x.l ?? '').join('')) }))(vba.mascara(m)));
    const fichas = g.fichas;
    if (g.data) return vba.formatoData(v, m);
    let x = vba.numero(v);
    if (Number.isNaN(x) || !fichas.some((f) => f.d)) return vba.str(v);
    if (fichas.some((f) => f.l === '%')) x *= 100;
    const ponto = fichas.findIndex((f) => f.p);
    const inteira = ponto < 0 ? fichas : fichas.slice(0, ponto);
    const decimal = ponto < 0 ? [] : fichas.slice(ponto + 1);
    const casas = decimal.filter((f) => f.d).length;
    let [int, frac = ''] = Math.abs(x).toFixed(casas).split('.');
    int = int.padStart(inteira.filter((f) => f.d === '0').length, '0');
    if (int === '0' && !inteira.some((f) => f.d === '0')) int = '';
    let parte;
    if (inteira.some((f) => f.m)) {
      // Com milhar: literais de antes, o número agrupado, literais de depois.
      const lit = (a) => a.map((f) => f.l ?? '').join('');
      const pd = inteira.findIndex((f) => f.d);
      const ud = inteira.length - 1 - [...inteira].reverse().findIndex((f) => f.d);
      parte = lit(inteira.slice(0, pd)) + int.replace(/\B(?=(\d{3})+(?!\d))/g, '.') + lit(inteira.slice(ud + 1));
    } else {
      // Os algarismos entram da direita para a esquerda nos 0/#; o que sobrar vai antes do primeiro.
      const digitos = int.split('');
      const saida = [];
      let antes = 0;
      for (let i = inteira.length - 1; i >= 0; i--) {
        const f = inteira[i];
        if (f.d) {
          saida.unshift(digitos.length ? digitos.pop() : '');
          antes = 0;
        } else if (f.l !== undefined) {
          saida.unshift(f.l);
          antes++;
        }
      }
      if (digitos.length) saida.splice(antes, 0, digitos.join(''));
      parte = saida.join('');
    }
    let dec = '';
    let k = 0;
    for (const f of decimal) dec += f.d ? frac[k++] ?? '' : f.l ?? '';
    return (x < 0 ? '-' : '') + parte + (casas ? ',' : '') + dec;
  },"#),
    ("mascara", &[], r#"  /** As fichas de uma máscara do Format: {d:'0'|'#'}, {p} (ponto), {m} (milhar), {l: literal}. */
  mascara(m) {
    const f = [];
    for (let i = 0; i < m.length; i++) {
      const c = m[i];
      if (c === '\\') f.push({ l: m[++i] ?? '' });
      else if (c === '"') { const j = m.indexOf('"', i + 1) < 0 ? m.length : m.indexOf('"', i + 1); f.push({ l: m.slice(i + 1, j) }); i = j; }
      else if (c === '0' || c === '#') f.push({ d: c });
      else if (c === '.') f.push({ p: true });
      else if (c === ',') f.push({ m: true });
      else f.push({ l: c });
    }
    return f;
  },"#),
    ("serial", &[], "  /** A data do VBA é a da planilha: dias desde 30/12/1899, com a hora na fração. As contas são em UTC para a hora local não deslocar o dia. */\n  serial(a, m = 1, d = 1, h = 0, n = 0, s = 0) { return (Date.UTC(a, m - 1, d, h, n, s) - Date.UTC(1899, 11, 30)) / 86400000; },"),
    ("agora", &["serial"], "  /** Now: a data e a hora do relógio local. */\n  agora() { const d = new Date(); return vba.serial(d.getFullYear(), d.getMonth() + 1, d.getDate(), d.getHours(), d.getMinutes(), d.getSeconds()); },"),
    ("cdate", &["serial", "numero", "str"], r#"  /** CDate: número fica; texto dd/mm/aaaa, aaaa-mm-dd (com hh:mm[:ss]) ou só hh:mm vira data. */
  _inteiro(s) { if (!s || s.length > 4) return NaN; for (let k = 0; k < s.length; k++) { const c = s.charCodeAt(k); if (c < 48 || c > 57) return NaN; } return Number(s); },
  _hms(h) { const p = h.split(':').map(vba._inteiro); return p.length >= 2 && p.length <= 3 && !p.some(Number.isNaN) ? p : null; },
  cdate(v) { if (typeof v === 'number') return v; if (typeof v === 'boolean') return v ? -1 : 0; if (v instanceof Date) return vba.serial(v.getFullYear(), v.getMonth() + 1, v.getDate(), v.getHours(), v.getMinutes(), v.getSeconds()); const t = vba.str(v).trim(); const x = vba.numero(t); if (!Number.isNaN(x)) return x; const inteiro = vba._inteiro, hms = vba._hms; let sp = t.indexOf(' '); if (sp < 0) sp = t.indexOf('T'); const data = sp < 0 ? t : t.slice(0, sp); const hora = sp < 0 ? '' : t.slice(sp + 1).trim(); if (!hora && data.indexOf(':') >= 0) { const h = hms(data); if (h) return (h[0] * 3600 + h[1] * 60 + (h[2] ?? 0)) / 86400; } const sep = data.indexOf('/') >= 0 ? '/' : data.indexOf('-') > 0 ? '-' : '.'; const partes = data.split(sep); const p = partes.map(inteiro); const h = hora ? hms(hora) : [0, 0, 0]; if (p.length === 3 && !p.some(Number.isNaN) && h) { let [y, mes, dia] = partes[0].length === 4 ? p : [p[2], p[1], p[0]]; if (y < 100) y += y < 30 ? 2000 : 1900; return vba.serial(y, mes, dia, h[0], h[1], h[2] ?? 0); } throw new TypeError(`Tipo incompatível: "${t}" não é data`); },"#),
    ("dp", &["cdate"], "  /** As partes da data (ler com getUTC…), arredondada ao segundo. */\n  dp(v) { return new Date(Date.UTC(1899, 11, 30) + Math.round(vba.cdate(v) * 86400) * 1000); },"),
    ("formatoData", &["dp", "str"], r#"  /** Format de data: yyyy yy mmmm mmm mm m dddd ddd dd d hh h nn ss; m/mm logo depois da hora é minuto, como no Excel. A máscara é lida uma vez e guardada. */
  _meses: ['janeiro', 'fevereiro', 'março', 'abril', 'maio', 'junho', 'julho', 'agosto', 'setembro', 'outubro', 'novembro', 'dezembro'],
  _dias: ['domingo', 'segunda-feira', 'terça-feira', 'quarta-feira', 'quinta-feira', 'sexta-feira', 'sábado'],
  _2(x) { return x < 10 ? '0' + x : '' + x; },
  formatoData(v, m) { m = vba.str(m); const guardadas = vba._datas ?? (vba._datas = {}); let fichas = guardadas[m]; if (!fichas) { fichas = []; const re = /yyyy|yy|mmmm|mmm|mm|dddd|ddd|dd|hh|nn|ss|m|d|h/gi; let r, i = 0, hora = false; while ((r = re.exec(m))) { if (r.index > i) fichas.push(m.slice(i, r.index)); let k = r[0].toLowerCase(); const minuto = hora && (k === 'mm' || k === 'm'); hora = k[0] === 'h'; if (minuto) k = k === 'mm' ? 'nn' : 'n'; fichas.push({ k }); i = re.lastIndex; } if (i < m.length) fichas.push(m.slice(i)); guardadas[m] = fichas; } const d = vba.dp(v); const meses = vba._meses, dias = vba._dias, p = vba._2; let s = ''; for (const f of fichas) { if (typeof f === 'string') { s += f; continue; } switch (f.k) { case 'yyyy': s += d.getUTCFullYear(); break; case 'yy': s += p(d.getUTCFullYear() % 100); break; case 'mmmm': s += meses[d.getUTCMonth()]; break; case 'mmm': s += meses[d.getUTCMonth()].slice(0, 3); break; case 'mm': s += p(d.getUTCMonth() + 1); break; case 'm': s += d.getUTCMonth() + 1; break; case 'dddd': s += dias[d.getUTCDay()]; break; case 'ddd': s += dias[d.getUTCDay()].slice(0, 3); break; case 'dd': s += p(d.getUTCDate()); break; case 'd': s += d.getUTCDate(); break; case 'hh': s += p(d.getUTCHours()); break; case 'h': s += d.getUTCHours(); break; case 'nn': s += p(d.getUTCMinutes()); break; case 'n': s += d.getUTCMinutes(); break; case 'ss': s += p(d.getUTCSeconds()); break; } } return s; },"#),
    ("weekday", &["dp", "n"], "  /** Weekday(data, primeiro dia): 1 a 7, domingo é 1 (vbSunday). */\n  weekday(v, p = 1) { return ((vba.dp(v).getUTCDay() + 8 - (vba.n(p) || 1)) % 7) + 1; },"),
    ("dateadd", &["cdate", "dp", "serial", "n"], "  /** DateAdd: d/y/w (dias), ww, h, n, s; m, q e yyyy param no último dia do mês (31/01 + 1 mês = 28/02). */\n  dateadd(i, n, v) { const x = vba.cdate(v); n = Math.trunc(vba.n(n)); const k = String(i).toLowerCase(); const seg = { d: 86400, y: 86400, w: 86400, ww: 604800, h: 3600, n: 60, s: 1 }[k]; if (seg) return (Math.round(x * 86400) + n * seg) / 86400; const meses = { m: 1, q: 3, yyyy: 12 }[k]; if (!meses) throw new Error(`DateAdd: intervalo \"${i}\" inválido`); const d = vba.dp(x); const total = d.getUTCFullYear() * 12 + d.getUTCMonth() + n * meses; const a = Math.floor(total / 12), m = total - a * 12; const ult = new Date(Date.UTC(a, m + 1, 0)).getUTCDate(); return vba.serial(a, m + 1, Math.min(d.getUTCDate(), ult), d.getUTCHours(), d.getUTCMinutes(), d.getUTCSeconds()); },"),
    ("datediff", &["cdate", "dp", "weekday"], "  /** DateDiff: quantas viradas do intervalo há de `a` até `b` (yyyy, q, m, d/y, w, ww, h, n, s). */\n  datediff(i, a, b, p = 1) { const x = vba.cdate(a), y = vba.cdate(b); const da = vba.dp(x), db = vba.dp(y); const s = (v) => Math.round(v * 86400); const sem = (v) => Math.floor((Math.floor(v) - vba.weekday(v, p) + 1) / 7); switch (String(i).toLowerCase()) { case 'yyyy': return db.getUTCFullYear() - da.getUTCFullYear(); case 'q': return (db.getUTCFullYear() - da.getUTCFullYear()) * 4 + Math.floor(db.getUTCMonth() / 3) - Math.floor(da.getUTCMonth() / 3); case 'm': return (db.getUTCFullYear() - da.getUTCFullYear()) * 12 + db.getUTCMonth() - da.getUTCMonth(); case 'd': case 'y': return Math.floor(y) - Math.floor(x); case 'w': return Math.trunc((Math.floor(y) - Math.floor(x)) / 7); case 'ww': return sem(y) - sem(x); case 'h': return Math.floor(s(y) / 3600) - Math.floor(s(x) / 3600); case 'n': return Math.floor(s(y) / 60) - Math.floor(s(x) / 60); case 's': return s(y) - s(x); } throw new Error(`DateDiff: intervalo \"${i}\" inválido`); },"),
    ("datepart", &["dp", "serial", "weekday", "datediff"], "  /** DatePart(intervalo, data): yyyy, q, m, y (dia do ano), d, w, ww, h, n, s. */\n  datepart(i, v, p = 1) { const d = vba.dp(v); const ano = d.getUTCFullYear(); switch (String(i).toLowerCase()) { case 'yyyy': return ano; case 'q': return Math.floor(d.getUTCMonth() / 3) + 1; case 'm': return d.getUTCMonth() + 1; case 'y': return Math.floor(vba.cdate(v)) - vba.serial(ano) + 1; case 'd': return d.getUTCDate(); case 'w': return vba.weekday(v, p); case 'ww': return vba.datediff('ww', vba.serial(ano), v, p) + 1; case 'h': return d.getUTCHours(); case 'n': return d.getUTCMinutes(); case 's': return d.getUTCSeconds(); } throw new Error(`DatePart: intervalo \"${i}\" inválido`); },"),
    ("dataTexto", &["dp", "formatoData"], "  /** A data como a pessoa digitaria na célula (a planilha a reconhece e põe o formato de data). */\n  dataTexto(v) { const x = vba.cdate(v); return vba.formatoData(x, x % 1 ? (Math.floor(x) ? 'dd/mm/yyyy hh:nn:ss' : 'hh:nn:ss') : 'dd/mm/yyyy'); },"),
    ("repetidas", &["coluna", "letra", "n"], "  /** `RemoveDuplicates Columns:=Array(1, 2), Header:=xlYes`: as colunas contam do começo do intervalo; sem `Header` é xlNo. */\n  repetidas(aba, e, cols, cab = 2) { const c0 = vba.coluna(e); const lista = cols === undefined ? undefined : (Array.isArray(cols) ? cols.slice(cols.lb ?? 0) : [cols]).map((c) => vba.letra(c0 + vba.n(c) - 1)); return aba.removeDuplicates(e, lista, { header: Number(cab) === 1 }); },"),
    ("condNova", &["numero"], "  /** `FormatConditions.Add`: a regra fica guardada até o estilo chegar (`.Interior.Color`, `.Font.Bold`). xlCellValue 1 compara com Formula1 (`=\"texto\"`, `=10`), xlExpression 2 é fórmula. */\n  condNova(aba, e, tipo = 1, op = 1, f1, f2) { const ops = { 1: 'between', 2: 'notBetween', 3: 'equal', 4: 'notEqual', 5: 'greaterThan', 6: 'lessThan', 7: 'greaterThanOrEqual', 8: 'lessThanOrEqual' }; const val = (f) => { if (f === undefined) return undefined; const t = String(f).trim(); if (!t.startsWith('=')) return f; const m = t.match(/^=\"((?:[^\"]|\"\")*)\"$/); if (m) return m[1].replace(/\"\"/g, '\"'); const x = vba.numero(t.slice(1)); return Number.isNaN(x) ? t : x; }; const o = Number(tipo) === 2 ? { type: 'formula', formula: String(f1) } : { operator: ops[Number(op)] ?? 'between', value: val(f1), value2: val(f2) }; const r = { aba, e, o }; (vba._regras ?? (vba._regras = [])).push(r); return r; },"),
    ("condAplicar", &[], "  /** Põe na planilha as regras de `FormatConditions.Add` guardadas. */\n  condAplicar() { for (const r of vba._regras ?? []) r.aba.addConditionalFormat(r.e, r.o); vba._regras = []; },"),
    ("condApagar", &["condAplicar"], "  /** `FormatConditions.Delete`: tira as regras que tocam o intervalo. */\n  condApagar(aba, e) { vba.condAplicar(); aba.clearConditionalFormats(e); },"),
    ("strconv", &["str"], r#"  /** StrConv: vbUpperCase 1, vbLowerCase 2, vbProperCase 3 (primeira letra de cada palavra maiúscula). */
  strconv(s, modo) { s = vba.str(s); switch (Number(modo)) { case 1: return s.toUpperCase(); case 2: return s.toLowerCase(); case 3: { const b = s.toLowerCase(); let r = '', ant = false; for (let i = 0; i < b.length; i++) { const ch = b[i]; const letra = ch !== ch.toUpperCase() || (ch >= '0' && ch <= '9') || ch === "'"; r += letra && !ant ? ch.toUpperCase() : ch; ant = letra; } return r; } } return s; },"#),
    ("msgBox", &["str", "n"], "  /** MsgBox: Sim/Não e OK/Cancelar pelo confirm; o resto pelo alert. Devolve vbYes (6), vbNo (7), vbOK (1) ou vbCancel (2). */\n  msgBox(texto, botoes = 0, titulo = '') { const t = (titulo ? vba.str(titulo) + '\\n\\n' : '') + vba.str(texto); const b = vba.n(botoes) & 7; if (b === 3 || b === 4) return confirm(t) ? 6 : 7; if (b === 1) return confirm(t) ? 1 : 2; alert(t); return 1; },"),
    ("inputBox", &["str"], "  inputBox(texto, titulo = '', padrao = '') { return prompt((titulo ? vba.str(titulo) + '\\n\\n' : '') + vba.str(texto), vba.str(padrao)) ?? ''; },"),
    ("partes", &[], "  /** \"$B$3:C5\" → [linha, coluna, linha, coluna] (do 1). Linha inteira (\"3:5\") e coluna inteira (\"B:D\") também. */\n  partes(e) { const ref = (r) => { const m = r.replace(/\\$/g, '').match(/^([A-Z]*)(\\d*)$/i); if (!m) throw new Error(`Endereço inválido: ${e}`); let c = 0; for (const ch of m[1].toUpperCase()) c = c * 26 + ch.charCodeAt(0) - 64; return [m[2] ? Number(m[2]) : 0, c]; }; const [a, b = a] = String(e).split('!').pop().split(':'); return [...ref(a), ...ref(b)]; },"),
    ("linha", &["partes"], "  /** A linha (do 1) da primeira célula do endereço. */\n  linha(e) { return vba.partes(e)[0] || 1; },"),
    ("coluna", &["partes"], "  /** A coluna (do 1) da primeira célula do endereço. */\n  coluna(e) { return vba.partes(e)[1] || 1; },"),
    ("letra", &[], "  letra(c) { let s = ''; for (let x = c; x > 0; x = Math.floor((x - 1) / 26)) s = String.fromCharCode(65 + ((x - 1) % 26)) + s; return s; },"),
    ("endAbs", &["partes", "letra"], "  /** O `.Address` do VBA: \"$B$3\". */\n  endAbs(e) { const [l0, c0, l1, c1] = vba.partes(e); const um = (l, c) => `$${vba.letra(c)}$${l}`; return l0 === l1 && c0 === c1 ? um(l0, c0) : `${um(l0, c0)}:${um(l1, c1)}`; },"),
    ("faixa", &["partes", "letra"], "  /** Range(a, b): do canto de cima à esquerda ao de baixo à direita. */\n  faixa(a, b) { const p = vba.partes(a); const q = vba.partes(b); return `${vba.letra(Math.min(p[1], q[1]))}${Math.min(p[0], q[0])}:${vba.letra(Math.max(p[3], q[3]))}${Math.max(p[2], q[2])}`; },"),
    ("celula", &["letra"], "  /** Cells(linha, coluna); a coluna pode ser número ou letra. */\n  celula(l, c) { return `${typeof c === 'string' && /^[A-Z]+$/i.test(c) ? c.toUpperCase() : vba.letra(Number(c))}${l}`; },"),
    ("mesclar", &[], "  /** `.Merge`: uma célula só não muda nada no Excel (o motor recusaria). */\n  mesclar(aba, a) { const e = String(a); const p = e.split(':'); if (p.length < 2 || p[0] === p[1]) return; aba.merge(e); },"),
    ("linhas", &["partes"], "  /** `.EntireRow`: \"A88:J92\" → \"88:92\". */\n  linhas(e) { const [l0, , l1] = vba.partes(e); return `${l0}:${l1}`; },"),
    ("colunas", &["partes", "letra"], "  /** `.EntireColumn`: \"B3:D9\" → \"B:D\". */\n  colunas(e) { const [, c0, , c1] = vba.partes(e); return `${vba.letra(c0)}:${vba.letra(c1)}`; },"),
    ("qtdLinhas", &["partes"], "  qtdLinhas(e) { const [l0, , l1] = vba.partes(e); return l1 - l0 + 1; },"),
    ("qtdColunas", &["partes"], "  qtdColunas(e) { const [, c0, , c1] = vba.partes(e); return c1 - c0 + 1; },"),
    ("like", &[], "  /** `texto Like padrão`: * ? # e [lista]. */\n  like(s, p) { const re = p.replace(/[.+^${}()|\\\\]/g, '\\\\$&').replace(/\\*/g, '.*').replace(/\\?/g, '.').replace(/#/g, '\\\\d').replace(/\\[!/g, '[^'); return new RegExp(`^${re}$`).test(s); },"),
    ("contar", &["partes"], "  /** `.Count` do intervalo: quantas células. */\n  contar(e) { const [l0, c0, l1, c1] = vba.partes(e); return (l1 - l0 + 1) * (c1 - c0 + 1); },"),
    ("celulas", &[], "  /** Os endereços das células do intervalo (o `For Each c In Range(…)`). */\n  celulas(aba, e) { const v = []; aba.range(e).forEach((_x, _l, _c, end) => v.push(end)); return v; },"),
    ("fim", &["partes", "letra", "vazio"], "  /** `.End(direção)` do VBA (Ctrl+seta): xlUp -4162, xlDown -4121, xlToLeft -4159, xlToRight -4161. */\n  fim(aba, e, dir) { const [l0, c0] = vba.partes(e); const [dl, dc] = { [-4162]: [-1, 0], [-4121]: [1, 0], [-4159]: [0, -1], [-4161]: [0, 1] }[dir]; const MAXL = 1048576, MAXC = 16384; const tem = (l, c) => !vba.vazio(aba.get(`${vba.letra(c)}${l}`)); const ultL = aba.getLastRow(vba.letra(c0)); const ultC = aba.getLastColumn(l0); let l = l0, c = c0; if (dl > 0 && l >= ultL) return `${vba.letra(c)}${MAXL}`; if (dc > 0 && c >= ultC) return `${vba.letra(MAXC)}${l}`; if (dl < 0 && l > ultL) { l = Math.max(ultL, 1); if (ultL) return `${vba.letra(c)}${l}`; } const dentro = (x, y) => x >= 1 && y >= 1 && x <= MAXL && y <= MAXC; if (tem(l, c) && dentro(l + dl, c + dc) && tem(l + dl, c + dc)) { while (dentro(l + dl, c + dc) && tem(l + dl, c + dc)) { l += dl; c += dc; } } else { do { l += dl; c += dc; } while (dentro(l + dl, c + dc) && !tem(l, c)); if (!dentro(l, c)) { l -= dl; c -= dc; } } return `${vba.letra(c)}${l}`; },"),
    ("corIndice", &[], "  /** A cor da paleta antiga do Excel (`ColorIndex`, 1 a 56). */\n  corIndice(i) { const p = ['#000000', '#ffffff', '#ff0000', '#00ff00', '#0000ff', '#ffff00', '#ff00ff', '#00ffff', '#800000', '#008000', '#000080', '#808000', '#800080', '#008080', '#c0c0c0', '#808080', '#9999ff', '#993366', '#ffffcc', '#ccffff', '#660066', '#ff8080', '#0066cc', '#ccccff', '#000080', '#ff00ff', '#ffff00', '#00ffff', '#800080', '#800000', '#008080', '#0000ff', '#00ccff', '#ccffff', '#ccffcc', '#ffff99', '#99ccff', '#ff99cc', '#cc99ff', '#ffcc99', '#3366ff', '#33cccc', '#99cc00', '#ffcc00', '#ff9900', '#ff6600', '#666699', '#969696', '#003366', '#339966', '#003300', '#333300', '#993300', '#993366', '#333399', '#333333']; return p[Number(i) - 1] ?? '#000000'; },"),
    ("semFundo", &[], "  /** Tira a cor de fundo (`Interior.Color = xlNone`). */\n  semFundo(aba, e) { aba.style(e, { bg: undefined }); },"),
    ("alinH", &[], "  /** xlLeft/xlCenter/xlRight/xlGeneral → o nome da API. */\n  alinH(v) { return { [-4131]: 'left', [-4108]: 'center', [-4152]: 'right', 1: 'general', 7: 'center' }[v] ?? 'general'; },"),
    ("alinV", &[], "  alinV(v) { return { [-4160]: 'top', [-4108]: 'middle', [-4107]: 'bottom' }[v] ?? 'bottom'; },"),
    ("matriz", &[], r#"  /** Dim v(n) / Dim v(1 To n, m): o índice é o do VBA (vai até n); `[1, n]` guarda o limite de baixo em `lb`. A última dimensão é criada num laço (a recursão por linha era lenta no motor das macros). */
  matriz(...lim) { const [d, ...resto] = lim; const [lb, n] = Array.isArray(d) ? d : [0, d]; const tam = Math.max(0, n + 1); const a = new Array(tam); if (resto.length === 1) { const [lb2, m] = Array.isArray(resto[0]) ? resto[0] : [0, resto[0]]; const largura = Math.max(0, m + 1); for (let i = 0; i < tam; i++) { const r = new Array(largura); if (lb2) r.lb = lb2; a[i] = r; } } else if (resto.length) { for (let i = 0; i < tam; i++) a[i] = vba.matriz(...resto); } if (lb) a.lb = lb; return a; },"#),
    ("lbound", &[], "  lbound(m, dim = 1) { let a = m; for (let i = 1; i < dim; i++) a = a[a.lb ?? 0]; return a.lb ?? 0; },"),
    ("ler", &["partes", "letra"], "  /** `Range.Value` de várias células: a matriz do VBA, `v(1 To linhas, 1 To colunas)`, lida em blocos (a API lê até 500 mil células por vez). */\n  ler(aba, e) { let [l0, c0, l1, c1] = vba.partes(e); if (l0 === l1 && c0 === c1 && l0) return aba.value(e); if (!l0 || !c0) { const u = aba.getDataRange(); if (!u) return aba.value('A1'); const p = vba.partes(u.address); if (!l0) { l0 = 1; l1 = p[2]; } if (!c0) { c0 = 1; c1 = p[3]; } } const m = [undefined]; m.lb = 1; const passo = Math.max(1, Math.floor(400000 / (c1 - c0 + 1))); for (let l = l0; l <= l1; l += passo) for (const r of aba.range(`${vba.letra(c0)}${l}:${vba.letra(c1)}${Math.min(l1, l + passo - 1)}`).values()) { r.unshift(undefined); r.lb = 1; m.push(r); } return m; },"),
    ("escrever", &["partes", "letra"], "  /** `Range.Value = matriz`: começa no limite de baixo de cada dimensão, recorta ao tamanho da área e grava em blocos. Matriz de uma dimensão é uma linha (repetida em cada linha da área, como no Excel). */\n  escrever(aba, e, v) { if (!Array.isArray(v)) { aba.set(e, v); return; } const [l0, c0, l1, c1] = vba.partes(e); const nl = l0 ? l1 - l0 + 1 : v.length, nc = c0 ? c1 - c0 + 1 : 16384; const linha = (r) => (Array.isArray(r) ? r.slice(r.lb ?? 0, (r.lb ?? 0) + nc) : [r]); const m = Array.isArray(v[v.lb ?? 0]) ? v.slice(v.lb ?? 0, (v.lb ?? 0) + nl).map(linha) : Array.from({ length: nl }, () => linha(v)); const passo = Math.max(1, Math.floor(400000 / Math.max(1, m[0]?.length ?? 1))); for (let i = 0; i < m.length; i += passo) aba.set(`${vba.letra(c0 || 1)}${(l0 || 1) + i}`, m.slice(i, i + passo)); },"),
    ("redim", &["matriz"], "  /** ReDim [Preserve]: a matriz nova, com os valores antigos se `preservar`. */\n  redim(antiga, preservar, ...lim) { const nova = vba.matriz(...lim); if (preservar && Array.isArray(antiga)) antiga.slice(0, nova.length).forEach((x, i) => { nova[i] = x; }); return nova; },"),
    ("ubound", &[], "  ubound(m, dim = 1) { let a = m; for (let i = 1; i < dim; i++) a = a[a.lb ?? 0]; return a.length - 1; },"),
    ("abas", &[], "  /** As abas da pasta, para o `For Each ws In Worksheets`. */\n  abas() { return sheet.sheets().map((n) => wb.sheet(n)); },"),
    ("hoje", &["agora"], "  hoje() { return Math.floor(vba.agora()); },"),
    ("ano", &["dp"], "  ano(v) { return vba.dp(v).getUTCFullYear(); },"),
    ("mes", &["dp"], "  mes(v) { return vba.dp(v).getUTCMonth() + 1; },"),
    ("dia", &["dp"], "  dia(v) { return vba.dp(v).getUTCDate(); },"),
    ("hora", &["dp"], "  hora(v) { return vba.dp(v).getUTCHours(); },"),
    ("minuto", &["dp"], "  minuto(v) { return vba.dp(v).getUTCMinutes(); },"),
    ("segundo", &["dp"], "  segundo(v) { return vba.dp(v).getUTCSeconds(); },"),
    ("arredBaixo", &["n"], "  /** WorksheetFunction.RoundDown. */\n  arredBaixo(x, casas = 0) { const f = 10 ** vba.n(casas); return Math.trunc(vba.n(x) * f) / f; },"),
    ("arredCima", &["n"], "  arredCima(x, casas = 0) { const f = 10 ** vba.n(casas); const v = vba.n(x) * f; return (v < 0 ? Math.floor(v) : Math.ceil(v)) / f; },"),
    ("erro", &[], "  /** O `Err` do VBA: o `catch` guarda o erro; `Err.Number`/`Err.Description` leem. */\n  erro(e) { vba._erro = e; },\n  erroNumero() { return vba._erro ? (vba._erro.number ?? 1) : 0; },\n  erroTexto() { return vba._erro ? String(vba._erro.message ?? vba._erro) : ''; },"),
    ("copiar", &[], "  /** `Range.Copy`/`.Cut` sem destino: guarda a origem para o `Paste` seguinte. */\n  copiar(aba, e, cortar = false) { vba._copia = { aba, e, cortar }; },"),
    ("colar", &[], "  /** `ActiveSheet.Paste` / `Range.PasteSpecial`: cola o último `Copy` (ou move o último `Cut`) a partir da primeira célula de `alvo`. */\n  colar(aba, alvo, modo = 'all') { const c = vba._copia; if (!c) throw new Error('Nada foi copiado antes do Paste'); const destino = String(alvo).split(':')[0]; const mesma = c.aba.name === aba.name; if (c.cortar && mesma) c.aba.move(c.e, destino); else { c.aba.copy(c.e, destino, mesma ? { paste: modo } : { paste: modo, sheet: aba.name }); if (c.cortar) c.aba.clear(c.e); } if (c.cortar) vba._copia = null; },"),
    ("novaPasta", &[], "  /** `Workbooks.Add`: a pasta nova da macro é a lista das abas copiadas para ela (`null` é a aba em branco que o Excel cria); o `SaveAs` grava só as copiadas. */\n  novaPasta() { return { abas: [null] }; },"),
    ("ordem", &[], "  /** O `Worksheet.Sort` do Excel: chaves, intervalo e cabeçalho guardados por aba até o `.Apply`. */\n  ordem(aba) { const m = vba._ordens ?? (vba._ordens = {}); return m[aba.name] ?? (m[aba.name] = { chaves: [], faixa: null, cabecalho: 0 }); },"),
    ("ordenar", &["ordem", "ordenarFaixa"], "  /** `.Sort.Apply`: ordena o `.SetRange` (ou a primeira chave) pelas chaves de `.SortFields.Add`. */\n  ordenar(aba) { const o = vba.ordem(aba); if (!o.chaves.length) return; vba.ordenarFaixa(aba, o.faixa ?? o.chaves[0][0], o.chaves, o.cabecalho); },"),
    ("ordenarFaixa", &["coluna", "letra", "partes", "numero", "vazio"], "  /** Ordena `faixa` por [[intervalo da chave, xlAscending 1 / xlDescending 2]]; cabeçalho xlYes 1, xlNo 2, xlGuess 0 (adivinha: linha 1 texto e linha 2 número). */\n  ordenarFaixa(aba, faixa, chaves, cab = 2) { const keys = chaves.filter(([k]) => k).map(([k, o]) => ({ col: vba.letra(vba.coluna(k)), order: Number(o) === 2 ? 'desc' : 'asc' })); let header = Number(cab) === 1; if (cab === undefined || Number(cab) === 0) { const l = vba.partes(faixa)[0] || 1; header = keys.some(({ col }) => { const a = aba.get(`${col}${l}`), b = aba.get(`${col}${l + 1}`); return typeof a === 'string' && !vba.vazio(a) && Number.isNaN(vba.numero(a)) && !vba.vazio(b) && !Number.isNaN(vba.numero(b)); }); } aba.sort(faixa, keys, { header }); },"),
    ("validar", &["n"], "  /** `Validation.Add Type, AlertStyle, Operator, Formula1, Formula2` → a validação da API. */\n  validar(aba, e, tipo, alerta = 1, op = 1, f1, f2) { tipo = Number(tipo); if (!tipo) { aba.clearDataValidation(e); return; } const mode = Number(alerta) === 1 ? 'stop' : 'warn'; const t = f1 === undefined ? '' : String(f1).trim(); let regra; if (tipo === 3) { regra = t.startsWith('=') ? { type: 'list', source: t } : { type: 'list', values: t.split(/[,;]/).map((s) => s.trim()).filter((s) => s) }; } else if ((tipo === 1 || tipo === 2) && !t.startsWith('=')) { const o = { 1: 'between', 2: 'notBetween', 3: 'equal', 4: 'notEqual', 5: 'greaterThan', 6: 'lessThan', 7: 'greaterThanOrEqual', 8: 'lessThanOrEqual' }[Number(op)] ?? 'between'; regra = /etween/.test(o) ? { type: 'number', operator: o, min: vba.n(f1), max: vba.n(f2) } : { type: 'number', operator: o, value: vba.n(f1) }; regra.integer = tipo === 1; } else { log(`Validação do Excel (tipo ${tipo}${t.startsWith('=') ? ', com fórmula' : ''}) ainda sem equivalente: ${e} ficou sem regra.`); aba.clearDataValidation(e); return; } regra.mode = mode; aba.setDataValidation(e, regra); },"),
];

/// O bloco `const vba = { … };` com as funções pedidas (e as dependências delas), em ordem
/// fixa. Vazio se nenhuma foi pedida.
pub fn bloco(pedidas: &BTreeSet<&'static str>) -> String {
    let mut todas: BTreeSet<&str> = BTreeSet::new();
    let mut fila: Vec<&str> = pedidas.iter().copied().collect();
    while let Some(n) = fila.pop() {
        if todas.insert(n) {
            if let Some((_, deps, _)) = AJUDAS.iter().find(|(x, _, _)| *x == n) {
                fila.extend(deps.iter().copied());
            }
        }
    }
    if todas.is_empty() {
        return String::new();
    }
    let corpo: Vec<&str> = AJUDAS.iter().filter(|(n, _, _)| todas.contains(n)).map(|(_, _, c)| *c).collect();
    format!("// O jeito do VBA de fazer as coisas, onde o JavaScript faz diferente.\nconst vba = {{\n{}\n}};\n", corpo.join("\n"))
}

/// Todas as ajudas (para os testes).
pub fn todas() -> String {
    bloco(&AJUDAS.iter().map(|(n, _, _)| *n).collect())
}

/// O nome existe na lista (para o gerador não pedir ajuda que não existe).
pub fn existe(nome: &str) -> bool {
    AJUDAS.iter().any(|(n, _, _)| *n == nome)
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn leva_as_dependencias_e_nada_mais() {
        let b = bloco(&BTreeSet::from(["eq"]));
        assert!(b.contains("eq(a, b)") && b.contains("cmp(a, b)") && b.contains("vazio(v)"));
        assert!(!b.contains("mid("));
        assert!(bloco(&BTreeSet::new()).is_empty());
    }

    #[test]
    fn toda_dependencia_existe() {
        for (n, deps, _) in AJUDAS {
            for d in *deps {
                assert!(existe(d), "{n} depende de {d}, que não existe");
            }
        }
    }
}
