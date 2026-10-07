"""在已初始化的隔离宿主中验证真实中间件行为"""
import json, time, os
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path
from client import *

login()
ids=json.loads(Path(os.environ["CPR_TEST_IDS"]).read_text())
passed=[]
def record(name): passed.append(name);print("PASS",name,flush=True)
def state():return api("GET",extension()+"state")
def settings(name,enabled=True,rule=None):
    s=state()
    return api("POST",extension()+"settings",{"version":s["version"],"account_id":ids[name]["account"],"enabled":enabled,"rule":rule or {"mode":"fixed","threshold":15}})
def request(name,stream=False):
    for _ in range(10):
        result=raw("POST","/v1/responses",{"model":"gpt-5.4","input":"test","stream":stream},{"Authorization":"Bearer "+ids[name]["key"]})
        if result[0]!=503 or b"account_capacity_unavailable" not in result[1]:return result
        time.sleep(.1)
    return result
def calls(name=None):
    return [e for e in control()["events"] if e["path"].endswith("/responses") and (name is None or e["account"]==name)]
def refresh_native(name):
    api("POST","/api/admin/accounts/quota/refresh",{"accountId":ids[name]["account"]})

control({"accounts":{"A":{"used":80},"B":{"used":20}}})
refresh_native("A");refresh_native("B")
settings("A")
control({"clear":True})
result=request("A");assert result[0]==200,result
record("首次使用 CPR 已有周额度，20% 剩余放行")

control({"accounts":{"A":{"used":90}}})
result=request("A");assert result[0]==200,result
count=len(calls("A"))
status,body=request("A")
assert status>=400,(status,body)
assert len(calls("A"))==count
assert state()["guards"][ids["A"]["account"]]["status"]=="额度保护暂停"
record("响应携带额度后，下次请求被拦截且没有上游推理")

result=request("B");assert result[0]==200,result
for _ in range(3):assert request("A")[0]>=400
assert len(calls("A"))==count
record("A 连续拒绝，B 被选中时仍可服务")

before=len([e for e in control()["events"] if e["path"].endswith("/usage")])
for _ in range(4):state()
settings("A")
assert len([e for e in control()["events"] if e["path"].endswith("/usage")])==before
record("刷新页面和保存规则未新增上游额度查询")

control({"accounts":{"A":{"used":85}}})
refresh_native("A")
result=request("A");assert result[0]==200,result
record("CPR 更新至恰好 15% 后自动恢复")

control({"accounts":{"A":{"used":90}}})
refresh_native("A")
assert request("A",True)[0]>=400
settings("A",False)
assert request("A",True)[0]==200
settings("A",True)
assert request("A")[0]>=400
record("SSE 拦截、关闭保护放行、同周期重新启用恢复拦截")

settings("A",True,{"mode":"fixed","threshold":5})
result=request("A");assert result[0]==200,result
record("修改固定阈值立即生效")

settings("B",True,{"mode":"random","min":10,"max":20})
value=state()["guards"][ids["B"]["account"]]["guard"]["threshold"]
assert 10<=value<=20
with ThreadPoolExecutor(max_workers=8) as pool:
    results=list(pool.map(lambda _:request("B"),range(16)))
assert any(status==200 for status,_ in results),results
assert all(status==200 or (status==503 and json.loads(body)["error"]["code"]=="account_capacity_unavailable") for status,body in results),results
with ThreadPoolExecutor(max_workers=8) as pool:
    snapshots=list(pool.map(lambda _:state(),range(16)))
assert all(s["guards"][ids["B"]["account"]]["guard"]["threshold"]==value for s in snapshots)
settings("B",False,{"mode":"random","min":10,"max":20})
settings("B",True,{"mode":"random","min":30,"max":40})
guard=state()["guards"][ids["B"]["account"]]["guard"]
assert guard["threshold"]==value and guard["current"]=={"mode":"random","min":10,"max":20}
record("并发、刷新、停启不重抽，随机区间等待下周期")

# 手动停用是宿主行为，插件刷新和配置保存不能撤销
api("POST","/api/admin/accounts/batch-update",{"accountIds":[ids["B"]["account"]],"enabled":False})
settings("B",True,{"mode":"random","min":30,"max":40})
assert not next(a for a in state()["accounts"] if a["account_id"]==ids["B"]["account"])["enabled"]
api("POST","/api/admin/accounts/batch-update",{"accountIds":[ids["B"]["account"]],"enabled":True})
record("不改变宿主手动停用状态")

control({"accounts":{"A":{"used":90}}})
refresh_native("A")
settings("A",True,{"mode":"fixed","threshold":15})
assert request("A")[0]>=400
i=api("GET","/api/admin/plugins/instances")[0]
api("POST","/api/admin/plugins/instances/disable",{"id":i["id"]})
result=request("A");assert result[0]==200,result
fields={k:i[k] for k in ["name","artifactSha256","configuration","bindings"]}
for binding in fields["bindings"]:binding.pop("identityBindings",None)
fields["enabled"]=True
api("POST","/api/admin/plugins/instances/update",{"id":i["id"],"instance":fields})
assert request("A")[0]>=400
assert state()["guards"][ids["B"]["account"]]["guard"]["threshold"]==value
record("停用插件后放行，重新启用恢复保护且保留随机阈值")

Path(os.environ.get("CPR_TEST_REPORT","validation/output/scenarios.json")).parent.mkdir(parents=True,exist_ok=True)
Path(os.environ.get("CPR_TEST_REPORT","validation/output/scenarios.json")).write_text(json.dumps({"passed":passed,"random_threshold":value},ensure_ascii=False,indent=2))
