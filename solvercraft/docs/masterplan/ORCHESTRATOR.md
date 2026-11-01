# Arbeitsauftrag für den Solvercraft Orchestrator

Entwickle Solvercraft nach MASTERPLAN.md und den verknüpften Anforderungen. Das Paket ist ein Entwicklungsplan, keine fertige Software. Die analysierte Gridcraft-Basis ist Commit 981fad14e0b90a7906a02f29785a6d0eb214fee6. Prüfe vor Arbeit den aktuellen Branch, alle zutreffenden AGENTS.md-Dateien und vorhandene lokale Planunterlagen. Erfinde keine nicht verfügbaren Dateien.

## Start

1. Ermittle Unterschiede zur Referenzbasis. Bestehende Nutzeränderungen bewahren. Kein automatischer Reset.
2. Lege ein Integrationsbranch und getrennte Agenten-Worktrees an. Halte pro Agent ein eigenes CARGO_TARGET_DIR.
3. Bearbeite SC-00 und SC-01; starte SC-02 nach dem ersten freigegebenen Modellvertrag. SC-25 integriert von Beginn an.
4. Friere die ersten API-Schemas, Crate-DAG, Ergebnissemantik und Owner-Pfade ein.
5. Delegiere freigegebene Aufgaben anhand tasks.json und prompts/. Nutze das verfügbare Parallelitätslimit; als Richtwert maximal sechs Worker plus Integrator, sofern die Umgebung dies erlaubt.
6. Ein Agent bearbeitet nur seine Owner-Pfade. Cargo/Lockfile, zentrale Root-Module, Layer-/Funktions-/Befehlsregistrierung und bestehende gemeinsame Dateien besitzt der Integrator.
7. Aufgaben ohne erforderliche Engine dürfen gegen einen Fake entwickeln. Dieser darf nicht in der Erfüllungsmatrix als Implementierung zählen.

## Laufende Arbeit

Führe den Auftrag autonom bis zu den erreichbaren Release-Gates aus. Fordere keine Routinebestätigung für reversible lokale Entwicklung. Stelle eine präzise Rückfrage nur bei fehlender produktrelevanter Entscheidung; arbeite an unabhängigen Paketen weiter. Veröffentliche oder verschicke nichts ohne entsprechenden Auftrag.

Jeder Agent erhält seine generierte Aufgabenaufforderung, die relevante Vertragversion und Requirements. Vertragsänderungen gehen mit Auswirkungen und Migration an SC-01/SC-25. Nicht alle 26 Aufgaben sofort starten: prerequisites müssen erfüllt sein. Die Wellenübersicht im Plan ist nur Orientierung; tasks.json ist für einzelne Pakete maßgeblich.

## Fachliche Regeln

- Rust und WASM bleiben Standard. Native/C++-Backends sind gesonderte optionale Projekte.
- Keine Frontline- oder Office-Binärdateien lesen, dekompilieren oder kopieren; keine proprietären Quellen/Assets übernehmen. Veröffentlichte Verhaltensbeschreibungen und zulässig lizenzierte Algorithmen verwenden.
- Eine beste gefundene zulässige Lösung ist kein Globaloptimum.
- Lokale NLP-Ergebnisse sind keine globalen Relaxierungsschranken.
- Originalmodellprüfung, Budgetierung, Fehlermeldungen, Abbruch und Revisionsschutz gehören zur Funktion.
- Kein globaler RNG-Zustand, der Scheduling oder wiederholte Formelzugriffe unkontrolliert verändert.
- Die Feature-Erfüllung bezieht sich auf die eingefrorene Referenzversion; keine erfundene Vollständigkeit.

## Prüfung und Übergabe

Führe pro Paket fachlich unabhängige Tests aus. Integriere kontrolliert und prüfe die vorhandenen Gridcraft-Gates. Ändere Feature-CI-Workflows nicht nebenbei. Halte erreichte Ergebnisse und offene Punkte im Statusbericht fest.

Jede Rückgabe nennt Commit, geänderte Dateien, ausgeführte Prüfungen mit echten Resultaten, numerische Garantien/Annahmen, Lizenzen, Ressourcen-/Plattformgrenzen und offene Requirements. SC-25 führt R1, R2, R3, R4 und R5 als gesonderte Freigaben; ein grüner Build allein genügt keinem Gate.

Liefere am Ende lauffähige Artefakte des erreichten Umfangs, eine wahrheitsgemäße Paritätsmatrix und eine Liste aller noch offenen Punkte. Lass kein Stub-Ergebnis als fertige Optimierung erscheinen.

