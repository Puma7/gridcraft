# Solvercraft Masterplan für Gridcraft und ArtCraft

**Stand:** 31. Oktober 2026  
**Ziel:** Wiederverwendbare Optimierungs- und Simulationsplattform in Rust mit Gridcraft als erster Anwendung.  
**Referenz:** legacy optimization and simulation workflows und Solver SDK.  
**Geprüfte Codebasis:** Puma7/gridcraft, main, Commit 981fad14e0b90a7906a02f29785a6d0eb214fee6, Workspace-Version 0.14.2.  
**Status:** Technischer Entwicklungsauftrag. Keine Solvercraft-Implementierung, kein ausgeführter Build und kein Leistungsnachweis.

## 1. Entscheidung und Produktziel

Solvercraft wird eine eigenständige Rust-Bibliothek mit einer dünnen Gridcraft-Anbindung. In Gridcraft erscheint im Daten-Menü unmittelbar rechts neben dem bestehenden Solver die Schaltfläche **Solvercraft**. Ihr Tooltip lautet „Erweiterte Optimierung und Simulation“. Ein rechtes Aufgabenpanel führt durch Modell, Methode, Simulation und Ergebnisse. Die Bibliothek funktioniert auch ohne Tabellenkalkulation und ohne egui, etwa für Ressourcenplanung, Layout, Rendering-Budgets oder parametrische Gestaltung in anderen ArtCraft-Anwendungen.

Das Ziel ist möglichst breite, nachgewiesene **Funktionsparität** für dokumentierte Optimierungs- und Simulationsverfahren. Der Code entsteht eigenständig aus überprüfbaren Schnittstellen und mathematischen Spezifikationen. Algorithmen werden aus veröffentlichter Fachliteratur eigenständig implementiert oder aus passend lizenzierten Projekten integriert. Namen, Grafiken, Handbuchtexte und UI-Gestaltung werden nicht kopiert. Maßstab sind mathematische Ergebnisse und Arbeitsabläufe, nicht binäre Identität oder identische Iterationsfolgen.

„Vollständig“ bedeutet: jede Anforderung im eingefrorenen Referenzkatalog ist implementiert und geprüft oder als konkrete, begründete Lücke ausgewiesen. Es bedeutet keine pauschale Unterstützung beliebiger Excel-Funktionen, beliebiger Modellgröße oder globaler Optimalität für jedes nichtkonvexe Problem. Ein Algorithmus mit lokalem Ergebnis wird auch so bezeichnet.

Ein gemeinsamer Entwicklungsauftrag kann alle Module enthalten. Integration und Freigabe erfolgen dennoch an überprüfbaren Zwischenständen. Parallelisierung ersetzt weder numerische Validierung noch Abhängigkeiten.

## 2. Verifizierter Iststand von Gridcraft

Die folgenden Punkte stammen aus dem Code des oben genannten Commits, nicht allein aus der README. Quellen G1–G8.

| Bereich | Vorhanden | Bedeutung für Solvercraft |
|---|---|---|
| Sprache und Oberfläche | Rust 2024, MSRV 1.95, egui/eframe; native und WASM-Anwendungen | Rust und WASM bleiben Standardanforderungen |
| Schichten | core/numfmt L0; formula/functions L1; model/calc/xlsx L2; chart L3; engine L5; UI/MCP L6 | Neue Crates müssen im Layer-Prüfer registriert werden |
| Bestehender Einstieg | data.solver und data.solverReport; Solver-Schaltfläche am Ende des Daten-Ribbons | Erweiterung daneben; alte API bleibt zunächst stabil |
| LP | Dichtes zweiphasiges Simplex, Dantzig/Bland; Koeffizienten durch n+1 Auswertungen und einen zusätzlichen Linearitätstest | Kleine Referenz und Migrationsbasis; keine hinreichende strukturelle Linearitätsprüfung |
| Ganzzahlprobleme | Tiefensuche mit Branch and Bound | Keine Gleichsetzung mit einem ausgereiften Branch-and-Cut-System |
| Nichtlinear | Augmented Lagrangian, projizierter Gradientenabstieg, zentrale Differenzen | UI-Name GRG beschreibt den tatsächlichen Algorithmus nicht |
| Nichtglatt | Genetische Suche mit Mutation/Crossover und nachgeschalteter Mustersuche | Nützliche Baseline; kein globaler Optimalitätsbeweis |
| Grenzen | 200 Variablen, 1000 skalare Constraints; Auswertungs- und Knotengrenzen | Nicht einfach Konstanten erhöhen: Datenstruktur und Arbeitsspeicher ändern |
| Modellbezüge | Entscheidungszellen und Constraints auf dem Blatt der Zielzelle | Blattübergreifende Bindungen neu modellieren |
| Speicherung | Versteckte blattbezogene solver_*-Namen | Import und kompatiblen Export des darstellbaren Teilumfangs erhalten |
| Berichte | Antwort- und Grenzberichte | Sensitivität und Dualinformationen gesondert ergänzen |
| Probeauswertung | Trial verwendet Workbook-Kopie und Calc; inkrementelle Neuberechnung | Semantische Referenz für neuen Snapshot-Evaluator |
| Dateierhalt | PreservedPart/PreservedRel/PreservedXml im Modell | Ansatzpunkt für eigene Paketbestandteile, kein Beweis fremder Roundtrip-Treue |
| Agentenzugang | Engine-Befehle über CLI, MCP und Kontrollkanal | Solvercraft bekommt dieselbe programmatische Reichweite |

**Wichtige Folgerungen:** Punktweise Tests können Nichtlinearität übersehen. Ein lokales NLP-Ergebnis liefert im Allgemeinen keine zulässige globale Schranke für ein nichtkonvexes Branch-and-Bound-Verfahren. Erfolgsstatus müssen deshalb aus nachprüfbaren Eigenschaften abgeleitet werden. Das sind Architektur- und Prüfpunkte; hier wurden keine konkreten Fehlermodelle ausgeführt.

Die Repository-Regeln verlangen unter anderem Result-basierte Fehler, keine Panics oder eigenes unsafe im Produktionscode, begrenzte Eingaben, Rust-only, WASM-Erhalt, dünne UI und Befehle für Nutzeraktionen. GPL/AGPL/LGPL-Code wird dort ausdrücklich ausgeschlossen. Neue CI-Workflows gehören nicht in den Feature-PR. G2.

## 3. Referenzumfang und belastbare Grenzen

Die interne Referenz umfasst konventionelle Optimierung, automatische Modellanalyse, zusätzliche Solver-Engines, Monte-Carlo-Simulation, Verteilungsanpassung, Korrelationen, Copulas, Entscheidungsbäume und Optimierung unter Unsicherheit. O1–O7. Dokumentierte Samplingverfahren sind Monte Carlo, Latin Hypercube und randomisierte Sobol-Verfahren. O8. Die PSI-Referenz unterscheidet Modellierungs-, Verteilungs-, Eigenschafts- und Statistikfunktionen. O10.

Die Webseiten nennen unterschiedliche Anzahlen von Verteilungen und Kennzahlen für verschiedene Produkte und Jahrgänge. Deshalb wird keine Zahl als angeblich eindeutige Vollständigkeitsgrenze übernommen. Der verbindliche Katalog wird in Arbeitspaket SC-00 aus einer festgelegten Produkt-/Handbuchversion gewonnen, mit einzelner Funktion, Signatur, Semantik, Quelle und Test-ID. Die öffentlich auffindbare Referenz 2025 Q1 überschritt beim Abruf das Größenlimit; sie wurde hier nicht vollständig ausgewertet. Der Engine-Guide 2025 Q4 und ausgewählte Online-Hilfen waren zugänglich.

| Referenzfamilie | Geplanter Umgang |
|---|---|
| Excel-Solver-Basismodelle | Migrationspflicht und Regressionstests |
| legacy PSI Optimization | Kernumfang |
| legacy PSI Simulation | Kernumfang |
| Kombination aus Optimierung und Simulation | Kernumfang der späteren Ausbaustufen |
| Solver SDK | Inspiration für eigenständige Modell- und Laufzeit-API |
| RASON/Server-Ausführung | Eigener versionierter JSON-Vertrag; RASON-Kompatibilität nur als gesondert geprüfter Importadapter |
| Data Science, Forecasting, Text Mining | Separates Erweiterungsprogramm; kein stillschweigendes Versprechen im Solver-Kern |
| Kommerzielle Plug-in-Engines | Optionales eigenes Adapterprojekt mit installierten, passend lizenzierten Engines |
| Proprietäre interne Verfahren | Keine Quellcode- oder Implementierungsidentität behaupten |

## 4. Funktionsumfang nach Ausbaustufe

Die maschinenlesbare Datei requirements.csv enthält die einzelnen Anforderungen samt Abnahme und zuständigem Paket. Die folgende Matrix fasst die Produktziele zusammen.

| Bereich | R1 Optimierung | R2 Simulation | R3 Risikooptimierung | R4 Erweiterungen | R5 Paritätsprüfung |
|---|---|---|---|---|---|
| Modellierung | Min/Max/Zielwert, Bounds, kontinuierlich/ganzzahlig/binär, mehrere Modelle, mehrere Blätter | Unsichere Eingaben und Ergebniszellen | Stufen, Risikoziele und Zufallsconstraints | Logik, SOS, indikatorbasierte und stückweise Modelle | Eingefrorener Funktionskatalog |
| Verfahren | Sparse LP/MILP, konvexe QP/SOCP, lokale glatte NLP, begrenzte nichtglatte Suche | MC/LHS, Verteilungen, Korrelation, Statistik | SAA, CVaR, Chance Constraints, robuste Gegenstücke, zweistufige Modelle | Randomized QMC, Dekomposition, spezialisierte globale und diskrete Verfahren | Unabhängige Validierung und dokumentierte Lücken |
| Bedienung | Aufgabenpanel, automatische Klassifikation, Fortschritt, Abbruch, Ergebnisübernahme | Verteilungseditor, Ergebnisgrafiken, Parametervarianten | Risikodialog, Trainings-/Validierungsszenarien | Entscheidungsbäume und vertiefte Diagnose | Beispiele, Hilfe, Plattformfreigabe |
| Analyse | Primalprüfung; passende Dual-/Basisinformationen, falls Engine fähig | Quantile, Tail-Risk, Sensitivität, Fitting | Out-of-sample-Berichte und Unsicherheit der Risikomaße | IIS/Konfliktanalyse, Mehrzielverfahren | Vergleichbare Benchmarkberichte |
| Wiederverwendung | Rust-SDK und JSON-Modell | Stapel- und Streamingausführung | Eigene risikobewusste Modell-API | Headless CLI; optionaler Jobserver | Versionierung und Migrationsgarantien |

R1 enthält die aufgeführten Klassen erst nach Bestehen ihrer jeweiligen Gates. Ein Release darf einen noch nicht freigegebenen Algorithmus als experimentell kennzeichnen; es darf ihn nicht als stabile Unterstützung zählen.

## 5. Mathematische Verfahren und Engine-Strategie

### 5.1 Standard bleibt reines Rust

**LP/MILP:** microlp ist ein Kandidat für den ersten austauschbaren Rust-Adapter. Das veröffentlichte Projekt beschreibt LP, Integer-/Binärvariablen und WASM und weist selbst auf numerische Grenzen hin. Version, Featurebaum, Fehlerverhalten, Abbruch und Ergebnisse werden vor Übernahme geprüft. E1. Keine Behauptung, es ersetze ohne Nachweis High-End-MIP-Engines.

**Konvexe QP/SOCP:** Clarabel.rs ist der erste Kandidat. Es unterstützt quadratische Zielfunktionen und konische Restriktionen unter Apache-2.0. Nur benötigte, WASM-kompatible Rust-Features aktivieren; optionale native Linearalgebra-/SDP-Pfade sind kein Standard. E2. Nichtkonvexe QP und gemischt-ganzzahlige QP/SOCP sind dadurch nicht automatisch abgedeckt.

**Glatte NLP:** zunächst bestehende Augmented-Lagrangian-Methode als ausdrücklich benannte Baseline extrahieren. Anschließend robuste Line Search/Trust Region, L-BFGS und ein SQP-Verfahren mit Restoration ergänzen. Vorhandene Rust-Optimierungsbausteine wie argmin werden als Komponenten geprüft, nicht als vollständiger Constraint-Solver vorausgesetzt. E4.

**Nichtglatte und globale Suche:** deterministische Mehrfachstarts, Pattern Search, Differential Evolution und optional CMA-ES. Nur als Heuristik mit bestem gefundenen zulässigem Punkt berichten. Intervall-Branch-and-Bound und konvexe Relaxierungen sind ein gesondertes R4/R5-Forschungs- und Entwicklungsprojekt.

**Diskret und gemischt:** MILP zuerst. Später SOS1/2, finite Domains, AllDifferent, Reifikation und spezialisierte Planungsmodelle. MINLP erfordert valide Relaxierungen für globale Schranken; lokale NLP-Ergebnisse dürfen keine unbegründeten Optimalitätsbeweise erzeugen.

**HiGHS:** geeignetes externes Vergleichssystem und möglicher optionaler Dienst, da LP/MIP/QP, MIT-Lizenz, aber C++-Implementierung. E3. Es wird nicht als reines Rust ausgegeben und nicht automatisch in Gridcraft eingebaut. Ebenso sind Gurobi, MOSEK, Xpress oder Knitro eigenständige lizenzierte Produkte.

### 5.2 Verbindlicher Fähigkeitenvertrag

Jeder Backend-Adapter meldet separat: Problemklassen, Integerfähigkeit, Warm Start, Dualwerte, Basis, Sensitivitätsintervalle, Konfliktzertifikate, Progress Callback, Abbruch, deterministischer Modus, WASM und überprüfbare Schranken. Die Auswahl erfolgt anhand dieses Vertrags. Unverfügbare Funktionen liefern einen konkreten Fehler; sie werden nicht still durch eine schwächere Methode ersetzt.

Backend-Version und aktive Features werden im Laufmanifest festgehalten. Keine Engine-Dependency wandert unbemerkt in den Standard-Featurebaum. Direkte und transitive Lizenzen, Notices und benötigte Native-Bibliotheken werden separat erfasst.

### 5.3 Modellerkennung

Der Compiler erzeugt eine eigene Expression-DAG mit Zellherkunft. Er propagiert Eigenschaften: konstant, affin, quadratisch, konvex/konkav unter bekannten Regeln, glatt, nichtglatt, stochastisch oder unbekannt. SUM und SUMPRODUCT sind affine Operatoren, wenn ihre Argumentabhängigkeiten das erlauben. Variable mal Variable ist quadratisch; variableabhängige IF-Zweige sind nicht pauschal linear.

Ein unbekannter Ausdruck darf als Black Box mit passender lokaler/heuristischer Methode laufen. Er bekommt keinen LP- oder Konvexitätsbeweis aus wenigen Stichproben. Für jeden automatischen Umbau gibt es Vorbedingungen und eine Herkunftsabbildung zurück zu Zellen und Constraints.

## 6. Wiederverwendbare Architektur

Die Namen und Schichten sind ein vorgeschlagener Zielzustand. SC-01 bestätigt sie mit dem bestehenden Layer-Prüfer.

| Crate | Schicht | Verantwortung | Darf nicht kennen |
|---|---:|---|---|
| solvercraft-types | L0 | IDs, numerische Optionswerte, serialisierbare Modelldaten, Status, Protokolltypen | Workbook, egui, Engine-Implementierungen |
| solvercraft-math | L1 | Sparse-Hilfen, Summierung, RNG, Verteilungen, Quantile, numerische Grundfunktionen | Gridcraft und UI |
| solvercraft-model | L1 | Tabellenunabhängige Variablen, Ausdrücke, Constraints und Ziele | Zelladressen und UI |
| solvercraft-compile | L2 | Klassifikation, Affin-/QP-Extraktion, Ableitungsgraph, Transformationen | Workbook und UI |
| solvercraft-engines | L3 | Adapter und LP/MILP/QP/NLP/Heuristik-Module | Workbook und UI |
| solvercraft-simulation | L3 | Sampling, Fitting, Korrelationen, Online-Statistik | Workbook und UI |
| solvercraft | L4 | Öffentliches SDK, Jobs, Validierung, Risikooptimierung, Entscheidungsbäume | Konkrete Spreadsheet-Oberfläche |
| solvercraft-gridcraft | L4 | Workbook-Snapshots, Zellbindungen, Formelübersetzung und Migrationsadapter | egui |
| gridcraft-engine | L5 | Befehle, Jobverwaltung, Transaktionen und Historie | Direkte UI-Ausführung |
| gridcraft-ui-egui / mcp | L6 | Bedienung bzw. Transport | Eigene mathematische Wahrheit |

Innerhalb L4 ist genau eine neue Reihenfolge nötig: solvercraft vor solvercraft-gridcraft. Diese wird ausdrücklich in INTRA_LAYER_ORDER eingetragen. Neue Paketnamen müssen in TABLE registriert werden; der bestehende Präfix-Parser erkennt solvercraft-Namen nicht automatisch. Sonstige Abhängigkeiten laufen abwärts. xlsx darf solvercraft-types benutzen, nicht den Solver oder calc; FORMAT_ALLOWED wird entsprechend gezielt ergänzt.

~~~mermaid
flowchart TD
  U["Gridcraft Panel und MCP"] --> E["Engine Befehle und Jobs"]
  E --> A["Gridcraft Adapter"]
  A --> W["Workbook Snapshot und Calc"]
  A --> S["Solvercraft SDK"]
  S --> C["Compiler und Modelldaten"]
  S --> O["Optimierungs Engines"]
  S --> M["Simulation und Statistik"]
~~~

Andere ArtCraft-Anwendungen implementieren eigene Bindungen an dieselbe Modell-/Evaluator-API. Der Solver-Kern enthält weder Workbook noch Zelladressen. Spreadsheet-Metadaten bleiben im Gridcraft-Adapter. Einheitliche Host-IDs erlauben später Bindungen an Layoutparameter, Keyframes oder Ressourcen.

## 7. API und Ergebnissemantik

Das öffentliche SDK bietet ModelBuilder, EngineRegistry, SolveRequest, SimulationRequest, RiskRequest, JobHandle und Evaluator. Evaluator nimmt einen Vektor von Entscheidungswerten und einen expliziten EvalContext entgegen und liefert Zielwerte, Constraintwerte und strukturierte Fehler. Ein Batch-Pfad erlaubt mehrere Szenarien ohne UI-Aufruf.

Der kompiliert-algebraische Pfad und der Black-Box-Pfad sind getrennte Fähigkeiten. Modelltypen sind von der Ausführungsumgebung unabhängig. Optionen sind versionierte Daten, keine untypisierten Schlüssel ohne Validierung.

Ein Ergebnis enthält mindestens:

| Teil | Pflichtinhalt |
|---|---|
| Identität | Modell-ID, Run-ID, Schema-/Engineversion, Snapshot-Hash und Workbook-Revision |
| Abbruchgrund | abgeschlossen, Zeitlimit, Auswertungslimit, Knotenlimit, Nutzerabbruch, Speicherlimit oder numerischer Fehler |
| Zulässigkeit | ungeprüft, zulässig innerhalb dokumentierter Toleranzen, verletzt oder nachgewiesen unzulässig |
| Optimalität | keine Aussage, stationärer Punkt, lokales Ergebnis, globale Schranke oder nachgewiesenes Optimum innerhalb Toleranzen |
| Zahlen | Zielfunktion, Variablen, größte Verletzung, verwendete Toleranzen; Schranke und Gap nur wenn valide |
| Diagnose | Zell-/Constraintherkunft, Iterationen, Auswertungen, Laufzeit, Warnungen |
| Nachvollziehbarkeit | Seed und RNG-/Samplingversion, aktive Features, Plattform und Reproduzierbarkeitsklasse |

„Kein zulässiger Punkt gefunden“ ist kein Unzulässigkeitsbeweis. „Algorithmus stagniert“ ist kein Nachweis eines lokalen Minimums. Ein überprüfter kleiner Gradient allein ersetzt keine Constraint- und Stationaritätsprüfung. Mathematische Zertifikate beziehen sich auf numerische Toleranzen; es werden keine exakten rationalen Beweise behauptet.

Für minimierte Ziele ist ein Gap erst mit zulässigem Incumbent und valider unterer Schranke definiert. Vorschlag: abs_gap = incumbent - bound; rel_gap = abs_gap / max(1, abs(incumbent), abs(bound)). Bei Maximierung entsprechend umkehren. Unterschiedliche Backenddefinitionen werden normalisiert und zusätzlich im Rohbericht bewahrt.

Grenzen werden im JSON als getaggte freie/untere/obere/beidseitige Schranken kodiert, nicht als Infinity oder NaN. 64-Bit-Seeds und IDs werden als Dezimalstrings übertragen, damit andere Clients sie nicht durch Gleitkommazahlen verkürzen.

## 8. Integration in Gridcraft

### 8.1 Konkrete Anknüpfungspunkte

| Datei oder Bereich | Geplante Änderung |
|---|---|
| crates/ui-egui/src/ribbon/data.rs | Solvercraft-Schaltfläche direkt nach vorhandenem Solver |
| crates/ui-egui/src/solvercraft/ | Neues Aufgabenpanel und Diagramm-/Verteilungsansichten |
| crates/ui-egui/src/app/commands.rs und state.rs | Panelzustand und Polling über Engine-API |
| crates/engine/src/cmd/solvercraft/ | Typisierte JSON-Befehle und Validierung |
| crates/engine/src/cmd/solver.rs | Später kompatibler Aufrufadapter; ursprüngliche API zunächst erhalten |
| crates/engine/src/cmd/whatif.rs | Trial als Referenz; gemeinsame Snapshot-Schnittstelle an geeigneter tieferer Stelle extrahieren |
| crates/calc/src/eval/ und recalc/ | Expliziter Auswertungskontext, deterministische Zufallswerte und Zeit |
| crates/model/ und crates/xlsx/ | Versionierte Modellspeicherung und sichere Zellbezugsmigration |
| crates/mcp/ und apps/gridcraft-cli/ | Jobstart, Status, Abbruch, Ergebnisausgabe |
| crates/engine/src/catalog.rs und UI-i18n | Auffindbarkeit, Hilfe und Übersetzungen |
| xtask/src/layers.rs | Neue Crates und erlaubte Abhängigkeiten |
| docs/architecture.md, roadmap.md, gaps.md, parity-Dateien | Implementierten Stand und bekannte Lücken nachführen |

### 8.2 Befehlsoberfläche

Vorgeschlagene Befehle: solvercraft.model.create/get/update/delete, solvercraft.validate, solvercraft.engines, solvercraft.solve, solvercraft.simulate, solvercraft.optimizeRisk, solvercraft.job.get/list/cancel, solvercraft.result.get/apply, solvercraft.report und solvercraft.model.import/export. Daten werden durch Schemas und Beispiele dokumentiert.

UI-only: ui.solvercraft.open. Ein leerer programmatischer Aufruf darf kein Fenster öffnen. Die neue API erfüllt dies ausdrücklich; die bestehende data.solver-Semantik wird in einem eigenen Kompatibilitätspaket behandelt.

Solve/Simulate starten Jobs und liefern Job-ID sowie Snapshot-Revision. CLI kann explizit bis zum Ende warten oder ein Zeitlimit nutzen. MCP-Status und Abbruch bleiben erreichbar, während ein Job arbeitet.

### 8.3 Benutzerablauf

1. Zielzelle oder Zielausdruck auswählen; Min, Max oder Zielwert festlegen.
2. Entscheidungsbereiche mit Typ und Grenzen zuordnen; Mehrblattbezüge unterstützen.
3. Constraints über Tabelleneditor oder Formelbindung anlegen.
4. Modell prüfen: Klasse, Größe, Skalierung, Fehler, unbekannte Funktionen und passende Engines anzeigen.
5. Für Simulation unsichere Eingaben, Abhängigkeiten und Ergebniszellen hinzufügen.
6. Job starten; beste Lösung, Zulässigkeit, Schranke und Zeitbudget sehen.
7. Ergebnis und geänderte Zellen prüfen; optional übernehmen oder Bericht erzeugen.

Die Übernahme ist ein einziger undo-fähiger Engine-Befehl. Vorher werden Workbook-Revision, Zellschutz, Formeln, Spills und Bindungen erneut geprüft. Bei Änderungen seit Jobstart wird die Übernahme abgelehnt; ein neuer Lauf oder eine ausdrückliche erneute Validierung ist nötig. Fehlschlag, Abbruch und Verwerfen verändern das Workbook nicht. Die UI kann während des Jobs weiter bedient werden.

### 8.4 Ausführung und Abbruch

Native Hosts verwenden Rust-Worker und begrenzte Nachrichtenkanäle. WASM erhält einen Rust-basierten Worker oder kooperative Poll-Ausführung; erzeugter Bindungscode ist zulässig, handgeschriebenes JS/TS ist kein Projektbestandteil. Jede Engine muss in begrenzten Schritten abgebrochen werden können. Backendaufrufe ohne überprüfbaren Abbruch werden nicht als interaktive Standardengine freigegeben.

Die Auswertungs-/Knotengrenze bleibt auch ohne verlässliche Uhr wirksam. Zeitmessung wird abstrahiert und auf WASM geprüft. Vorschlag für die Abnahme: sichtbarer Status spätestens alle 250 ms, Bedienreaktion unter 100 ms bei normalen Modellen und Abbruch innerhalb 1 s an definierten Abbruchpunkten. Diese Werte sind Ziele, noch keine Messwerte.

## 9. Formelauswertung und automatische Ableitungen

Snapshot-Auswertungen dürfen Live-Workbook und Historie nicht verändern. Statische Abhängigkeiten werden vorab erfasst, Konstanten gefaltet und nur relevante Formelteile berechnet. Der neue schnelle Pfad wird gegen den vorhandenen Calc/Trial-Pfad geprüft. Bis zur Gleichwertigkeit ist letzterer der semantische Rückfallpfad.

Der EvalContext enthält feste Zeit, Seed, Trial-/Szenario-ID, Zufallsbindung, Fehlerpolitik und Rechenbudget. NOW/TODAY werden pro Lauf eingefroren; RAND/RANDBETWEEN werden explizit eingefroren, durch modellierte Zufallsquellen ersetzt oder beim deterministischen Solve abgelehnt. Externe Links werden als Snapshotwerte ausgewiesen, Makros/UDFs nicht still ausgeführt.

INDIRECT, OFFSET, variableabhängige Lookups, LET/LAMBDA, Arrays und Spills brauchen eine Fähigkeitsliste. Dynamische Referenzen werden konservativ behandelt und zur Laufzeit nachgeprüft. Zirkuläre Bezüge werden anfangs abgelehnt; spätere iterative Unterstützung braucht eine definierte Fixpunktsemantik und Fehlerbehandlung. Keine stille Ersetzung von Formel-, Domain- oder Missing-Fehlern durch Null.

Vorwärts-/Rückwärtsdifferentiation wird für unterstützte DAG-Operatoren ergänzt. Finite Differenzen bleiben kontrollierter Rückfall mit skalierungsabhängigen Schritten, Domainprüfung und Ableitungsvergleich. Nichtglatte Stellen bekommen einen anderen Methodenvorschlag. Symbolische Affin-/QP-Extraktion hat Vorrang vor numerischer Koeffizientenschätzung.

Formeln der neuen Familie SC.NORMAL, SC.TRIANGULAR, SC.OUTPUT, SC.MEAN, SC.QUANTILE und SC.CVAR sind ein Vorschlag. SC.* wird zunächst im Calc-Spezialfunktionspfad behandelt; eine functions-L1-Abhängigkeit auf simulation-L3 wäre verboten. Gemeinsame Datentypen dürfen aus types-L0 kommen. Ein neues Kontextobjekt ist keine Mutation globaler Registry-Zustände.

## 10. Monte-Carlo-Simulation und Statistik

### 10.1 Verteilungen

R2 beginnt mit Uniform, diskret Uniform, Normal, Lognormal, Dreieck, Beta-PERT, Beta, Gamma, Exponential, Weibull, Bernoulli, Binomial, Poisson, Negativ-Binomial, Geometrisch und empirisch diskret/stetig. Für jede Verteilung sind Parameterkonvention, Support, Randfälle, Sample, CDF, inverse CDF und verfügbare Momente spezifiziert.

R4 ergänzt nach eingefrorenem Referenzkatalog unter anderem Student-t, Chi-Quadrat, F, Cauchy, Logistic, Laplace, Gumbel, Pareto, Rayleigh, Erlang, inverse Gamma, inverse Gaussian, Loglogistic, Johnson-Familien, Skew-Normal, Hypergeometrisch, kategoriale und weitere erforderliche Familien. Trunkierung, Verschiebung, Skalierung, Mischungen, zusammengesetzte Verteilungen und Parameter aus Perzentilen werden als eigene Fähigkeiten gezählt, nicht zum Aufblähen einer Verteilungszahl.

Zieldimension ist die vollständige einzeln getestete Referenzliste. Für numerisch schwierige Tails oder undefinierte Momente gibt es eine explizite Rückgabe statt irreführender Zahlen.

### 10.2 Zufall und Abhängigkeit

MC und LHS zuerst; randomisierte/scrambled Sobol-Samples und geeignete Varianzreduktion später. Eine geprüfte counter-basierte RNG-Strategie adressiert Zufallswerte über Lauf, Modellbindung, Simulation, Trial, Replikat und Subcounter. Threadzahl und Scheduling verändern die Samples nicht. Seed und RNG-Version gehören zum Manifest.

Mehrfaches Lesen derselben Zufallszelle in einem Trial liefert denselben Wert. Unterschiedliche Zellen mit identischer Formel erhalten eigene Streams, außer wenn eine gemeinsame Bindung gewünscht ist. Dimensionszuordnung bei QMC und LHS ist stabil und wird gespeichert. Die Lizenz der Sobol-Richtungstabellen wird geprüft.

R2 unterstützt Rangkorrelation und Gaussian Copula; R4 ergänzt Student-t, Clayton, Frank und Gumbel mit gültigen Parameterbereichen und Dimensionen. O9. Korrelationsmatrizen werden auf Symmetrie, Diagonale und positive Semidefinitheit geprüft. Eine Reparatur verändert das Modell sichtbar und benötigt eine bewusste Auswahl. Pearson-Korrelation, Rangkorrelation und Copula-Parameter werden nicht verwechselt.

### 10.3 Ergebnisse und Fitting

Online-Auswertung: Anzahl gültiger/fehlerhafter Trials, Mittelwert, Varianz, Standardabweichung, Minimum/Maximum, Median, Quantile, Schiefe/Kurtosis bei Existenz, CDF, Überschreitungswahrscheinlichkeiten, VaR und Expected Shortfall/CVaR mit ausdrücklich definierter Verlustkonvention. Bei diskreten Verteilungen wird die Masse an der Quantilsgrenze korrekt behandelt.

Histogramme, CDF, Boxplots, Quantilbänder, Scatterplots, Overlay und Tornado-Diagramme. Rangbasierte Sensitivität, Regression und Einzelfaktor-Stresstests werden unterschiedlich beschriftet; Sensitivität ist keine Kausalitätsaussage.

Fitting unterstützt MLE bzw. passende Schätzverfahren, AIC/BIC, KS/AD-Diagnose, Q-Q-Plots, Bootstrap, empirische Verteilungen und nachvollziehbare Modellwahl. Datenqualität, fehlende Werte, Censoring/Trunkierung und Gewichtung werden explizit modelliert. Bei geschätzten Parametern gelten angepasste bzw. bootstrapbasierte Güteprüfungen; naive Standard-p-Werte sind nicht ausreichend.

Resultate können gestreamt und in begrenzten Chunks gespeichert werden. Exakte Quantile benötigen gespeicherte Daten oder externe Sortierung; Sketches werden als Approximation mit Parametern gekennzeichnet. Es gibt keinen Zwang, Trials × Eingaben × Ausgaben vollständig im RAM zu halten.

Konfidenzintervalle für IID-MC werden nicht ungeprüft auf LHS oder einen einzelnen Sobol-Lauf angewendet. Randomized QMC nutzt unabhängige Scrambles/Replikate. Eine vorzeitige Stoppregel muss ihre Gültigkeitsannahmen dokumentieren.

## 11. Risikooptimierung, Szenarien und Entscheidungsbäume

**Parameterläufe:** ein Modell über Wertelisten oder Parametergrids lösen/simulieren; eindeutige Achsen und Warm-Start-Regeln; Ergebnisse vergleichbar speichern.

**Simulation Optimization:** Sample Average Approximation mit festen Trainingsszenarien bzw. Common Random Numbers für faire Kandidatenvergleiche. Optimierung und unabhängige Validierung nutzen getrennte Seeds. Zunächst Erwartungswert, Varianzstrafe und CVaR; später weitere monotone Risikomaße. Optimizer-Rauschen und Monte-Carlo-Unsicherheit erscheinen getrennt.

**Chance Constraints:** einzelne und gemeinsame Wahrscheinlichkeitsbedingungen unterscheiden. Szenariobasierte Approximationen können MILP-Reformulierungen benötigen. Big-M nur aus begründeten endlichen Grenzen ableiten, niemals als magische Konstante. Validierungsverletzungen und einseitige Binomialintervalle ausweisen. Empirische Erfüllung ist keine allgemeine Wahrscheinlichkeitsgarantie.

**Robuste Optimierung:** explizite Box-, Budget- und ellipsoidale Unsicherheitsmengen. Dokumentierte LP-/SOCP-Gegenstücke gelten nur bei erfüllten Strukturannahmen. Eine robuste Menge ist kein automatisch kalibrierter Ersatz für eine Wahrscheinlichkeitsbedingung.

**Stochastische Programmierung:** zunächst zweistufige lineare Modelle mit Recourse als deterministisches Äquivalent. Entscheidungen vor der Unsicherheit werden über Szenarien identisch gehalten; spätere Entscheidungen dürfen nur verfügbare Informationen verwenden. Danach Benders/L-Shaped für den unterstützten konvex-linearen Umfang, später mehrstufige Szenariobäume und weitere Dekomposition. Ganzzahlige Recourse und nichtkonvexe Teilprobleme sind gesonderte Fähigkeiten. Simulationsoptimierung und Modelle mit Recourse haben getrennte Abnahmekriterien.

**Entscheidungsbäume:** Entscheidungs-, Zufalls- und Endknoten; Wahrscheinlichkeitsprüfung; Rückwärtsauswertung, Erwartungsnutzen, Risikoprofile, Sensitivität und Value of Information. Simulierte Payoffs sind möglich. Baumdiagramm und Szenariobaum der Optimierung bleiben fachlich unterscheidbar.

**Mehrzieloptimierung:** gewichtete Ziele, lexikografische Prioritäten und epsilon-Constraints. Pareto-Punkte sind nur unter dokumentierten Solverannahmen vollständig; bei nichtkonvexen Modellen kann eine gewichtete Summe Teile der Front verfehlen.

## 12. Speicherung und Kompatibilität

Der portable Kern speichert versioniertes solvercraft.model.json mit Modell, Optionen und Metadaten. Große Trialdaten bleiben getrennte, begrenzte Ergebnisartefakte; kein impliziter Workbook-Bloat.

Gridcraft speichert die Modellmetadaten typisiert im Workbook über types-L0. Vorgeschlagen ist ein eigener OPC-Paketbestandteil unter xl/solvercraft/ mit registriertem Content-Type und Relationship vom Workbook-Part. SC-06 prüft den tatsächlich passenden Anschluss an den vorhandenen XLSX-Reader/Writer. Eigene Teile müssen zwischen Gridcraft-Lese-/Schreibzyklen verlustfrei bleiben. Excel oder andere Programme können unbekannte Teile entfernen; deshalb gibt es einen expliziten Sidecar-Export.

Jede Bindung erhält eine persistente ID und strukturelle Zellreferenzen. Einfügen/Löschen, Umbenennen/Verschieben/Kopieren von Blättern, Tabellenänderungen und benannte Bereiche werden getestet. Fehlt im vorhandenen Workbookmodell eine stabile Blatt-ID, wird diese gezielt ergänzt; numerische Blattindizes allein reichen nicht.

Bestehende solver_*-Modelle werden eingelesen. Nur der darstellbare Teilumfang wird zurückgeschrieben, mit Meldung über ausgelassene Solvercraft-Funktionen. SC.*-Formeln werden in normalem Excel ohne Add-in nicht automatisch ausführbar. Optionaler Export von Ergebniswerten und kompatiblen Basisformeln ist möglich; keine Behauptung vollständiger Excel-/legacy-Dateikompatibilität.

Psi*-Kompatibilität wird funktionsweise eingeführt: dokumentierte Signatur, Defaultparameter, Reihenfolge der Argumente, Verhalten außerhalb der Simulation und Fehlerfälle erhalten eigene Tests. Beispiel: PsiTriangular verwendet laut Online-Hilfe die Reihenfolge Minimum, Modus, Maximum. O11. Ein pauschaler Namensalias genügt nicht. Unbekannte Funktionen bleiben erhalten und werden verständlich als nicht unterstützt gemeldet.

## 13. Stabilität, Genauigkeit und Ressourcen

Jeder Lauf hat Grenzen für Variablen, Constraints, Nichtnullkoeffizienten, DAG-Größe, Rekursion, Trials, Szenarien, Iterationen, Knoten, Zeit, Ergebnisgröße und geschätzten Arbeitsspeicher. Diese Grenzen sind konfigurierbar und werden vor großen Allokationen überprüft. Keine pauschale Zusage „unbegrenzt“.

Skalierung und Toleranzen sind Modellbestandteil. Jede ausgegebene Lösung wird im Originalmodell nachgerechnet: Bounds, Integrality, Constraints und Zielwert. Gerundete Integerwerte werden erneut validiert; sie werden nicht automatisch als zulässig angenommen. Numerisch zweifelhafte Ergebnisse erhalten einen passenden Status.

LP/MILP-Numerik: Presolve, gebundene Variablen, freie Variablen, degenerierte Basen, fast abhängige Constraints, große Koeffizientenbereiche und Primal-/Dualresiduen prüfen. Ein Backendfehler bleibt ein Backendfehler und wird nicht als infeasible umgedeutet.

Der Reproduzierbarkeitsvertrag unterscheidet identische Samples, wiederholbare Ausführung auf derselben Plattform und numerische Äquivalenz über Architekturen. Floating-Point-Reduktionen und parallele Engines können plattformabhängig abweichen; bitidentische Ergebnisse werden nur für entsprechend geprüfte Pfade zugesagt.

Logs enthalten standardmäßig Status und Kennzahlen, nicht ganze Geschäftsmodelle oder Zellinhalte. Netzwerk- und Cloudausführung sind ausdrücklich gewählte Laufzeitoptionen. Der normale Solver arbeitet lokal und benötigt keinen Lizenzserver.

## 14. Abnahme und Benchmarkprogramm

| Prüfbereich | Verbindliche Prüfung |
|---|---|
| Bestehender Solver | Vorhandene solver/tests.rs plus API-, Modell- und Berichtsmigration |
| LP/MILP | Synthetische Probleme mit analytischen Optima; kleine Integerprobleme erschöpfend prüfen; lizenzgeprüfte Benchmarkfälle |
| QP/SOCP | KKT-/Residualprüfung, bekannte konvexe Lösungen, Nichtkonvexitätsablehnung |
| NLP | Analytische Gradienten, AD-vs-Differenzen, Domainfehler, verschiedene Startpunkte, Stationarität und Feasibility getrennt |
| Nichtglatt/global | Feste Seeds, definierte Budgets, Qualität statt unzulässigem Optimalitätsversprechen |
| Compiler | Gegenbeispiele gegen Stichprobenlinearität; Herkunftsabbildung; Transformationsgleichwertigkeit |
| Simulation | Verteilungseigenschaften, Quantile und Randfälle; deterministische Ensembles gegen analytische Referenzen |
| Korrelation | Reproduzierbare Abhängigkeiten und ungültige Matrizen; Copula-Tailverhalten |
| Risiko | Kleine Newsvendor-/Recourse-Modelle mit berechenbarer Referenz; getrennte Trainings-/Validierungsszenarien |
| Workbook | Zell-/Blattoperationen, XLSX-Roundtrip, Undo/Redo, geänderte Revision während Job |
| Runtime | Abbruch, leere Parameter, Fehler, Speichergrenzen, kein blockierter Statuszugang |
| Plattformen | Native Zielplattformen und wasm32; echte Browserausführung zusätzlich zum Kompilieren |
| Reuse | Zweiter minimaler Host ohne gridcraft- und egui-Abhängigkeiten |
| Qualität | cargo xtask ci und betroffene spezifische Tests; keine pauschale Test-Pass-Behauptung |

HiGHS und andere unabhängige Solver dienen bei verfügbaren Lizenzen als Differentialreferenzen. Mehrere identische optimale Zielfunktionswerte können verschiedene Variablenbelegungen haben. Verglichen werden Zulässigkeit, Zielwert, Schranken und Status innerhalb dokumentierter Toleranzen, nicht identische Lösungsvektoren.

Kleine numerische Referenzen werden in Rust erzeugt. Fremde Corpora werden nur lizenzgeprüft, gepinnt und hashverifiziert verwendet. legacy-generierte oder Microsoft-eigene Dateien/Assets werden nicht in das Repository kopiert; black-box Vergleichsläufe sind getrennt von selbst erzeugten Regressionstests.

**Vorgeschlagene Leistungsklassen, noch ungemessen:** klein 100 Variablen/500 Constraints; mittel 10.000/50.000 mit etwa 500.000 Nichtnullen; Simulation 100.000 Trials mit 20 Eingaben und 10 Ausgaben. Schwere MIP-Instanzen bekommen keinen allein von ihrer Dimension abhängigen Laufzeitanspruch. Für jede Klasse werden Hardware, Modellstruktur, Peak-RAM, Fehlerrate und Zeit bis zur ersten zulässigen Lösung erfasst. Erst danach entstehen konkrete SLA-Werte.

Ein Benchmarkbericht nennt Code-/Engineversion, Compiler, Hardware, Seed, Parallelität und Budgets. Geschwindigkeitsvergleiche brauchen reproduzierbare Messungen.

## 15. Umsetzung mit Subagenten

Die Datei tasks.json enthält 26 Pakete SC-00 bis SC-25. Jede Aufgabe hat Owner-Pfade, Abhängigkeiten, Lieferobjekte, Tests und einen ausführbaren Arbeitsauftrag als Text. prompts/ enthält daraus einzelne Übergabeaufforderungen. Der Orchestrator beginnt mit SC-00/01 und vergibt weitere Pakete erst, wenn ihre vertraglichen Voraussetzungen vorliegen.

| Welle | Parallelisierbare Pakete | Integrationsziel |
|---|---|---|
| A | SC-00 Referenzkatalog, SC-01 Architekturverträge, SC-02 unabhängige Testreferenzen | G0 Spezifikation und nachweisbare Baseline |
| B | SC-03 Formelauswertung, SC-04 Modellcompiler, SC-06 Persistenz, SC-07 LP-Adapter, SC-12 UI-Gerüst | G1 Kernverträge; R1-Arbeit vorbereitet |
| C | SC-05 Ableitungen, SC-08 QP/SOCP, SC-09 NLP, SC-10 Heuristik, SC-11 Jobs, SC-14 RNG/Verteilungen | R1-Gate über SC-13 und SC-25 |
| D | SC-15 Sampling/Abhängigkeiten, SC-16 Fitting/Statistik, SC-17 Simulation-UX | R2-Gate |
| E | SC-18 Simulation Optimization, SC-19 robust/chance, SC-20 Recourse | R3-Gate |
| F | SC-21 Entscheidungsbäume/Mehrziel, SC-22 global/diskret, SC-23 Kompatibilität/Server, SC-24 zweiter Host | R4 und R5 nach Funktionsnachweisen |
| fortlaufend | SC-02 und SC-25 Validierung/Integration | Jeder Release hat eigene Prüfbasis |

Die Wellen sind eine Übersicht, kein Ersatz für die genaueren Task-Abhängigkeiten. Besonders SC-11 und SC-14 können nach ihren Verträgen früher starten. Kein Agent darf gemeinsam genutzte Verträge eigenmächtig umdefinieren.

### Arbeitsregeln für die Durchführung

- Ein Integrationsverantwortlicher besitzt Workspace-Cargo, Cargo.lock, Layer-Regeln und zentrale Befehlsregistrierung. Andere Agenten liefern dafür gezielte Änderungsvorschläge.
- Agenten arbeiten in getrennten Branches/Worktrees mit eigenem CARGO_TARGET_DIR. Gemeinsame Arbeitsverzeichnisse sind kein Ersatz für Isolation.
- Jeder Agent besitzt festgelegte Module. Änderungen an fremden Owner-Pfaden werden über den Integrator abgestimmt.
- Gemeinsame Datentypen werden vor Implementierung eingefroren; Vertragsänderungen erhalten Versionsnummer, Migration und betroffene Pakete.
- Test-Fakes sind erlaubt, gelten aber nicht als mathematische Implementierung. Ein Stub darf niemals Optimalität oder gültige Statistiken melden.
- Jede Aufgabe endet mit Dateien, Tests, echten Messergebnissen, Quellen/Lizenzen, verbleibenden Lücken und einem reviewbaren Commit/PR.
- Eine unabhängige Validierung kontrolliert Ergebnisse und Garantieaussagen. „Kompiliert“ und „sieht fertig aus“ sind keine Freigabekriterien.
- Feature Flags halten unfertige Teile außerhalb stabiler Standardpfade. Gates gelten pro Funktion, nicht nur pro Paket.
- Keine Issues, Nachrichten, PRs oder Veröffentlichungen sind mit diesem Plan bereits angelegt oder versendet worden.

## 16. Gates und Definition of Done

**G0:** Referenzversion, Featurekatalog, API-Schemas, Crate-DAG, Lizenzmatrix und numerische Referenzen stehen fest. Fehlende lokale plan/-Dokumente werden nicht erfunden; falls sie beim Implementieren verfügbar sind, werden sie zuerst gelesen.

**G1 / R1:** Modelle laufen über UI, CLI und MCP; LP/MILP, freigegebene QP/SOCP/NLP/Heuristiken haben überprüfte Fähigkeiten; asynchrone Jobs sind abbrechbar; Originalmodellprüfung und atomare Übernahme funktionieren; bestehende Solvermodelle gehen nicht verloren; WASM läuft.

**G2 / R2:** Referenzbasis der Verteilungen, MC/LHS, definierte Abhängigkeiten, Streamingstatistik und Ergebnisgrafiken bestehen ihre Prüfungen. Gleiches Seedmanifest erzeugt dieselben Samples unabhängig von Workerzahl.

**G3 / R3:** Trainings-/Validierungstrennung, CVaR, deklarierte Chance-/Robust-Annahmen und zweistufige Nichtantizipativität sind geprüft. Statistische Unsicherheit wird dargestellt.

**G4 / R4:** Fortgeschrittene Funktionen sind einzeln validiert; ihr mathematischer Geltungsbereich ist in API und Hilfe identisch.

**G5 / Parität:** Jede Zeile des eingefrorenen Referenzkatalogs hat Implementierung, Test, Plattformstatus und einen eindeutigen Erfüllungsgrad. Ungeprüfte oder fehlende Features zählen nicht als erfüllt. Dokumentation, Beispielmodelle und Migration sind vollständig; Stabilitäts- und Benchmarkberichte sind reproduzierbar.

Definition of Done für jede Funktion: implementiert, ohne Dummy-Ergebnis, fachlich geprüft, begrenzt und abbrechbar, mit nachvollziehbaren Fehlern, lizenzierten Abhängigkeiten, dokumentierter Garantie und passenden Regressionstests; relevante Gridcraft-Gates bestanden.

## 17. Aufwand, Risiken und Startentscheidung

Die folgenden Bandbreiten sind Planungsannahmen für ein Team mit erfahrener Rust-Entwicklung und mindestens einer Person für numerische Optimierung/Statistik. Sie sind keine aus diesem Repository gemessenen Schätzungen. Die Aufgaben werden nach SC-00/01/02 neu geschätzt.

| Lieferstufe | Erste Planungsbandbreite |
|---|---|
| Architektur und Engine-Evaluierung | 2–4 Wochen |
| R1 nach freigegebenen Verträgen | weitere 6–12 Wochen mit etwa 3–4 erfahrenen Verantwortlichen |
| R2 | weitere 8–12 Wochen, zum Teil parallel |
| R3 | weitere 10–16 Wochen |
| R4 und robuste Paritätsprüfung | zusätzliche 4–9 Monate, abhängig von diskreten/globalen Anforderungen |
| Breite, belastbare Plattform | grob 9–18 Monate Kalenderzeit; etwa 30–70 Personenmonate als zu validierende Größenordnung |

Agenten können Implementierung und Testentwicklung beschleunigen. Sie verkürzen die notwendige Validierung schwieriger numerischer Verfahren nicht auf einen einzigen Lauf. Volle globale MINLP-/Constraint-Programming-Parität kann ein eigenes längerfristiges Solverprojekt werden.

| Risiko | Gegenmaßnahme |
|---|---|
| Rust-Backend erfüllt schwierige MIP- oder Numerikanforderungen nicht | Früh mit repräsentativen Fällen prüfen; Adapter austauschbar halten; gezielte Engineverbesserung budgetieren |
| Schneller Formelevaluator verändert Semantik | Gegen Calc/Trial prüfen; konservativer Rückfall |
| Gemeinsame Dateiänderungen blockieren Agenten | Verträge und Ownership; Integrator kontrolliert Registrierungen |
| Simulation optimiert zufälliges Rauschen | Feste Trainingsszenarien, unabhängige Validierung, Unsicherheitsberichte |
| Fremdprogramme entfernen Modellmetadaten | Eigener Sidecar und klarer Roundtrip-Bericht |
| „Global optimal“ ohne Beweis | Statusvertrag und unabhängige Schrankenprüfung |
| Funktionszahl wächst ohne Abschluss | Eingefrorene Referenzversion und versionierter Erweiterungskatalog |
| UI friert bei Solveraufruf ein | Abbruchfähige Backends, Jobs und Browserlauf-Gate |

**Empfohlener erster Entwicklungsauftrag:** SC-00, SC-01 und SC-02 starten; danach einen vertikalen, echten LP-Lauf vom Panel bis zur transaktionalen Übernahme liefern. Parallel dürfen Typen, Compiler, Persistenz, Engine-Adapter und UI auf dem eingefrorenen Vertrag entstehen. Simulation und Risikooptimierung benutzen anschließend denselben geprüften Auswertungspfad.

## 18. Quellen und Belegstatus

Alle Webquellen wurden am 31.10.2026 konsultiert. Repositoryaussagen beziehen sich auf den festgehaltenen Commit. Architektur, Prioritäten, Fristen, APIs und Akzeptanzziele dieses Plans sind eigene Vorschläge. Produktseiten beschreiben Herstelleraussagen; sie sind keine unabhängigen Geschwindigkeitsmessungen.

### Gridcraft

- G1 Repository und Cargo.toml: https://github.com/Puma7/gridcraft/tree/981fad14e0b90a7906a02f29785a6d0eb214fee6
- G2 Agentenregeln: https://github.com/Puma7/gridcraft/blob/981fad14e0b90a7906a02f29785a6d0eb214fee6/AGENTS.md
- G3 Solver und Optionen: https://github.com/Puma7/gridcraft/blob/981fad14e0b90a7906a02f29785a6d0eb214fee6/crates/engine/src/cmd/solver.rs
- G4 Solver-Untermodule: https://github.com/Puma7/gridcraft/tree/981fad14e0b90a7906a02f29785a6d0eb214fee6/crates/engine/src/cmd/solver
- G5 Trial: https://github.com/Puma7/gridcraft/blob/981fad14e0b90a7906a02f29785a6d0eb214fee6/crates/engine/src/cmd/whatif.rs
- G6 Ribbon: https://github.com/Puma7/gridcraft/blob/981fad14e0b90a7906a02f29785a6d0eb214fee6/crates/ui-egui/src/ribbon/data.rs
- G7 Layer-Prüfer: https://github.com/Puma7/gridcraft/blob/981fad14e0b90a7906a02f29785a6d0eb214fee6/xtask/src/layers.rs
- G8 Calc und Erhalt fremder Teile: crates/calc/src/recalc.rs und crates/model/src/preserved.rs am selben Commit.

### Interne Kompatibilitätsreferenzen

- O1 Produktübersicht: interne Referenz
- O2 Optimierung: interne Referenz
- O3 Simulation: interne Referenz
- O4 Produktvergleich: interne Referenz
- O5 Solver SDK Engines: interne Referenz
- O6 Engine-Guide 2025 Q4: interne Referenz
- O7 Stochastische Klassen und Recourse: interne Referenz und Engine-Guide 2025 Q4; Abschnitte zur stochastischen Optimierung.
- O8 Sampling: interne Referenz
- O9 Copulas: interne Referenz
- O10 PSI-Familien: interne Referenz ; neuere Referenz 2025 Q1 identifiziert, hier nicht vollständig abgerufen.
- O11 Dreiecksverteilung: interne Referenz

### Wiederverwendbare Bausteine

- E1 microlp: https://github.com/Specy/microlp und https://docs.rs/crate/microlp/latest ; Apache-2.0 laut Projekt. Übernahmekandidat, noch kein ausgewählter/pinnierter Produktionsstand.
- E2 Clarabel: https://clarabel.org/stable/ und https://github.com/oxfordcontrol/Clarabel.rs ; Apache-2.0 laut Projekt. Features und WASM-Ausführung vor Übernahme prüfen.
- E3 HiGHS: https://highs.dev/ und https://github.com/ERGO-Code/HiGHS ; MIT, C++ laut Projekt.
- E4 argmin: https://www.argmin-rs.org/book/ ; Komponentenframework, keine hier nachgewiesene vollständige NLP-Engine.

Die Datei sources.json unterscheidet gelesene Repositorydateien, geöffnete Dokumentation, Suchbelege und unvollständig verfügbare Referenzen. Eine zusätzliche Prüfung sämtlicher Handbuchfunktionen ist als SC-00 geplant und wird nicht als bereits erledigt ausgegeben.

