"""隔离环境管理客户端，仅访问本机测试端口"""
import json, urllib.request, urllib.error, http.cookiejar, os
BASE = os.environ.get("CPR_TEST_BASE", "http://127.0.0.1:9080")
jar = http.cookiejar.CookieJar()
opener = urllib.request.build_opener(urllib.request.HTTPCookieProcessor(jar))
def raw(method, path, data=None, headers=None):
    headers = dict(headers or {})
    if isinstance(data, (dict, list)):
        data = json.dumps(data).encode();headers["Content-Type"] = "application/json"
    req = urllib.request.Request(BASE+path, data=data, headers=headers, method=method)
    try:
        with opener.open(req, timeout=30) as r:return r.status, r.read()
    except urllib.error.HTTPError as e:return e.code,e.read()
def api(method,path,data=None):
    status,body=raw(method,path,data)
    value=json.loads(body)
    if status>=400:raise RuntimeError((path,status,value))
    if isinstance(value,dict) and "data" in value:return value["data"]
    return value
def login():
    api("POST","/api/auth/login",{"mode":"admin","username":"admin@quota.test","password":"quota-guard-test-only"})
    # 隔离网段使用 HTTP，测试客户端显式保留仅用于此环境的会话
    for cookie in jar:
        cookie.secure = False
def control(data=None):
    req=urllib.request.Request(BASE.replace(":9080",":9081")+"/control",data=json.dumps(data).encode() if data else None,headers={"Content-Type":"application/json"})
    return json.load(urllib.request.urlopen(req))
def extension():
    ex=api("GET","/api/admin/plugins/extensions")[0]["target"]
    return "/api/admin/plugins/extensions/"+"/".join(str(ex[k]) for k in ["instanceId","artifactSha256","revision"])+"/api/"
