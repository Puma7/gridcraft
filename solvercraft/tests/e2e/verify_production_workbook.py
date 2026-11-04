"""End-to-end workbook verification using the Gridcraft command transport."""
from pathlib import Path
import json
import math
import queue
import statistics
import subprocess
import sys
import threading
import tempfile
import time

import numpy as np
from scipy.optimize import Bounds, LinearConstraint, milp

ROOT = Path(__file__).resolve().parents[2]
BOOK = ROOT / "examples" / "production-12-weeks.xlsx"
PLANS = ["Heute", "Kostenplan", "Reserveplan"]


class Client:
    def __init__(self, executable):
        self.lines = queue.Queue()
        self.counter = 0
        self.process = subprocess.Popen([str(executable), "mcp"], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True, encoding="utf8")
        threading.Thread(target=lambda: [self.lines.put(line) for line in self.process.stdout], daemon=True).start()
        self.rpc("initialize", {"protocolVersion":"2025-06-18", "capabilities":{}, "clientInfo":{"name":"production-e2e","version":"1"}})
        self.rpc("notifications/initialized", notify=True)

    def rpc(self, method, params=None, notify=False):
        self.counter += 1
        packet = {"jsonrpc":"2.0", "method":method}
        if not notify:
            packet["id"] = self.counter
        if params is not None:
            packet["params"] = params
        self.process.stdin.write(json.dumps(packet)+"\n")
        self.process.stdin.flush()
        if notify:
            return
        while True:
            response = json.loads(self.lines.get(timeout=90))
            if response.get("id") == self.counter:
                break
        if "error" in response:
            raise RuntimeError(response["error"])
        return response["result"]

    def tool(self, name, args):
        result = self.rpc("tools/call", {"name":name, "arguments":args})
        if result.get("isError"):
            raise RuntimeError(result)
        return json.loads(result["content"][0]["text"])

    def cmd(self, name, args=None):
        return self.tool("execute_command", {"command":name,"params":args or {}})

    def read(self, sheet, area):
        self.cmd("sheet.activate", {"sheet":sheet})
        return self.tool("read_range", {"range":area})["values"]

    def put(self, sheet, cell, value):
        self.cmd("sheet.activate", {"sheet":sheet})
        self.tool("set_cell", {"cell":cell,"input":str(value)})

    def run(self, **args):
        job = self.cmd("solvercraft.job.start", {**args,"limits":{"max_time_millis":60000}})
        ref = {"id":job["id"]}
        deadline = time.monotonic()+75
        while True:
            status = self.cmd("solvercraft.job.get", ref)
            if status["status"] not in ["queued","running"]:
                break
            if time.monotonic() > deadline:
                raise TimeoutError(status)
            time.sleep(.05)
        assert status["status"] == "completed", status
        return ref, self.cmd("solvercraft.job.result", ref)

    def close(self):
        self.process.stdin.close()
        try:
            assert self.process.wait(timeout=12) == 0
        except subprocess.TimeoutExpired:
            self.process.kill()
            self.process.wait()
            raise


def oracle(data, reserve, available):
    # Independent formulation: stock is eliminated algebraically, variables are
    # ordered by product, then week. HiGHS solves this matrix independently.
    demand = np.array(data["demand"])
    lot, holding, overtime = [np.array(data[k]) for k in ("lotCost","stockCost","overtimeCost")]
    holding_offset = np.sum(np.cumsum(demand, axis=1)*holding[:,None])
    c = np.r_[(lot[:,None]+holding[:,None]*np.arange(12,0,-1)).ravel(),np.repeat(overtime,12)]
    matrix, low, high = [], [], []
    def add(entries, lower, upper):
        row = np.zeros(108)
        for column,value in entries:
            row[column] = value
        matrix.append(row)
        low.append(lower)
        high.append(upper)
    for p in range(6):
        for w in range(12):
            add([(12*p+i,1) for i in range(w+1)], sum(demand[p,:w+1])+reserve, np.inf)
    for line in range(3):
        for w in range(12):
            add([(24*line+w,1),(24*line+12+w,1),(72+12*line+w,-1)],-np.inf,available[line][w]-data["setup"][line][w])
    for w in range(12):
        add([(12*p+w,1) for p in range(6)],-np.inf,data["material"][w])
    bounds=Bounds(np.zeros(108),np.r_[np.full(72,60),np.full(36,20)])
    result=milp(c,integrality=np.ones(108),bounds=bounds,constraints=LinearConstraint(matrix,low,high),options={"time_limit":30,"mip_rel_gap":0})
    assert result.success and result.mip_gap == 0, result
    return float(result.fun-holding_offset)


def check_plan(data, production, overtime, available, reserve):
    production, overtime = np.array(production), np.array(overtime)
    for matrix, upper in ((production,60),(overtime,20)):
        assert np.all(matrix == np.rint(matrix)) and np.all(matrix >= 0) and np.all(matrix <= upper)
    stock = np.cumsum(production-np.array(data["demand"]),axis=1)
    assert np.all(stock >= reserve)
    assert np.all(production.sum(axis=0) <= data["material"])
    assert np.all(production.reshape(3,2,12).sum(axis=1) <= np.array(available)-data["setup"]+overtime)
    return int(np.sum(production*np.array(data["lotCost"])[:,None]) + np.sum(stock*np.array(data["stockCost"])[:,None]) + np.sum(overtime*np.array(data["overtimeCost"])[:,None]))


def simulate_one(data, production, randoms, settings):
    threshold, _, swing, delay, material_factor, outage, line_factor = settings
    demand = data["demand"]
    delivered, total = 0.0, 0.0
    stock = [0.0]*6
    for week in range(12):
        material = material_factor if randoms[1][week] < delay else 1.0
        for p in range(6):
            line = line_factor if randoms[2+p//2][week] < outage else 1.0
            demand_trial = demand[p][week]*(1+swing*(2*randoms[0][week]-1))
            total += demand_trial
            stock[p] += production[p][week]*material*line
            sold = min(stock[p],demand_trial)
            delivered += sold
            stock[p] -= sold
    quote = delivered/total
    return quote, float(quote < threshold)


def wilson(values):
    proportion, n = statistics.mean(values), len(values)
    z = 1.959963984540054
    center = (proportion+z*z/(2*n))/(1+z*z/n)
    radius = z*math.sqrt(proportion*(1-proportion)/n+z*z/(4*n*n))/(1+z*z/n)
    return max(0.0,center-radius), min(1.0,center+radius)


def verify_simulation(client, model_id, request, data, plans, settings, *, extra=True):
    ref, original = client.run(model_id=model_id)
    samples = original["result"]["samples"]
    assert original["result"]["termination"] == "completed", original["result"]["termination"]
    assert len(samples)==request["options"]["trials"]
    if extra:
        all_outputs = [*request["outputs"], *(x["cell"] for x in request["inputs"])]
        expanded_ref, expanded = client.run(kind="simulation", request={**request,"outputs":all_outputs})
        for sample, full in zip(samples,expanded["result"]["samples"],strict=True):
            assert sample == full[:6]
            draws = [full[6+i*12:18+i*12] for i in range(5)]
            for index,plan in enumerate(plans):
                quote, indicator = simulate_one(data,plan,draws,settings)
                assert math.isclose(quote,sample[2*index],abs_tol=2e-14) and indicator == sample[2*index+1], (quote,indicator,sample)
        client.cmd("solvercraft.job.forget",expanded_ref)
    records = []
    for i in range(3):
        quote = [row[i*2] for row in samples]
        missed = [row[i*2+1] for row in samples]
        for j,values in enumerate((quote,missed)):
            summary = original["result"]["summaries"][i*2+j]
            assert math.isclose(summary["mean"],statistics.mean(values),abs_tol=1e-12)
            assert math.isclose(summary["variance"],statistics.variance(values),abs_tol=1e-12)
            ordered = sorted(values)
            for probability, quantile in summary["quantiles"]:
                index = max(0, math.ceil(probability * len(ordered)) - 1)
                assert quantile == ordered[index]
        records.append({"mean":statistics.mean(quote),"miss_probability":statistics.mean(missed),"wilson95":wilson(missed)})
    return ref,original,records


def main():
    if len(sys.argv) != 2:
        raise SystemExit("Usage: python verify_production_workbook.py PATH_TO_GRIDCRAFT_CLI")
    client = Client(Path(sys.argv[1]).resolve())
    try:
        client.tool("open_workbook", {"path": str(BOOK)})
        models = client.cmd("solvercraft.model.list")["models"]
        assert len(models) == 3
        ids = [next(m["id"] for m in models if m["name"].startswith(f"{i} ·")) for i in (1, 2, 3)]
        request = client.cmd("solvercraft.model.get", {"id":ids[2]})["request"]
        data = {
            "demand": client.read("Annahmen", "B7:M12"),
            "available": client.read("Annahmen", "B15:M17"),
            "setup": client.read("Annahmen", "B19:M21"),
            "material": client.read("Annahmen", "B23:M23")[0],
            "lotCost": [row[0] for row in client.read("Annahmen", "B26:B31")],
            "stockCost": [row[0] for row in client.read("Annahmen", "C26:C31")],
            "overtimeCost": client.read("Annahmen", "B33:D33")[0],
        }
        settings = [row[0] for row in client.read("Annahmen", "B37:B43")]
        current = client.read("Heute", "B7:M12")
        assert check_plan(data, current, client.read("Heute","B15:M17"), data["available"], 0) == client.read("Heute","B4")[0][0]

        def optimize(available, expected):
            plans = [current]
            for i, sheet in enumerate(PLANS[1:]):
                before = client.read(sheet, "B7:M12")
                ref, preview = client.run(model_id=ids[i])
                assert client.read(sheet, "B7:M12") == before
                result = preview["result"]
                assert result["feasibility"] == "feasible" and result["optimality"] == "optimal", result
                assert abs(result["objective"] - oracle(data, settings[1]*i, available)) < 1e-7
                assert result["objective"] == expected[i]
                client.cmd("sheet.activate", {"sheet":"Annahmen"})
                client.cmd("solvercraft.job.apply", ref)
                after = client.read(sheet, "B7:M12")
                assert check_plan(data, after, client.read(sheet, "B15:M17"), available, settings[1]*i) == expected[i]
                assert client.read(sheet, "B4") == [[expected[i]]]
                assert client.read(sheet, "E4") == [[0]]
                report = client.cmd("solvercraft.job.report", {**ref, "language":"de"})
                assert client.read(report["sheet"], "B8") == [[expected[i]]]
                client.cmd("edit.undo")
                assert client.read(sheet, "B7:M12") == after
                client.cmd("edit.undo")
                assert client.read(sheet, "B7:M12") == before
                client.cmd("solvercraft.job.forget", ref)
                ref, _ = client.run(model_id=ids[i])
                client.cmd("solvercraft.job.apply", ref)
                assert client.read(sheet, "B7:M12") == after
                client.cmd("solvercraft.job.forget", ref)
                plans.append(after)
            return plans

        plans = optimize(data["available"], [100498,103238])
        ref, _, original = verify_simulation(client, ids[2], request, data, plans, settings)
        report = client.cmd("solvercraft.job.report", {**ref, "name":"Testlauf", "language":"de"})
        assert client.read(report["sheet"], "C5") == [[request["options"]["trials"]]]
        assert abs(client.read(report["sheet"], "B7")[0][0] - original[0]["mean"]) < 1e-12
        client.cmd("solvercraft.job.forget", ref)

        with tempfile.TemporaryDirectory() as directory:
            saved = Path(directory) / "verification.xlsx"
            client.tool("save_workbook", {"path":str(saved)})
            client.tool("open_workbook", {"path":str(saved)})
            assert len(client.cmd("solvercraft.model.list")["models"]) == 3
            assert client.read("Testlauf", "C5") == [[2048]]
            assert client.read("Reserveplan", "B7:M12") == plans[2]

        stale, _ = client.run(model_id=ids[0])
        client.put("Annahmen","G16",20)
        try:
            client.cmd("solvercraft.job.apply", stale)
        except RuntimeError as error:
            assert "changed" in str(error), error
        else:
            raise AssertionError("A stale optimization could still be applied")
        client.cmd("solvercraft.job.forget", stale)
        available = [row.copy() for row in data["available"]]
        available[1][5] = 20
        changed_plans = optimize(available, [100828,103568])
        ref, _, changed = verify_simulation(client, ids[2], request, data, changed_plans, settings)
        client.cmd("solvercraft.job.forget", ref)

        # Convergence and plan ordering with two additional seeds and budgets.
        replications = []
        for seed, count in [("20261104",1024), ("20261105",4096)]:
            options = {**request["options"], "seed":seed, "trials":count}
            ref, preview = client.run(kind="simulation", request={**request, "options":options})
            assert preview["result"]["completed_trials"] == count
            misses = [preview["result"]["summaries"][i]["mean"] for i in (1,3,5)]
            assert misses[0] < misses[2] < misses[1]
            for observed, reference in zip(misses, changed, strict=True):
                assert abs(observed-reference["miss_probability"]) < 0.08
            replications.append({"seed":seed, "trials":count, "miss_rates":misses})
            client.cmd("solvercraft.job.forget", ref)

        for cell in ("B39","B40","B42"):
            client.put("Annahmen",cell,0)
        for i in (2,3,5):
            settings[i] = 0
        ref, _, analytic = verify_simulation(client, ids[2], request, data, changed_plans, settings, extra=False)
        client.cmd("solvercraft.job.forget", ref)
        assert all(x["mean"] == 1 and x["miss_probability"] == 0 for x in analytic)
        print(json.dumps({"initial":original,"capacity_change":changed,"replications":replications,"no_shocks":analytic},indent=2))
    finally:
        client.close()


if __name__ == "__main__":
    main()
