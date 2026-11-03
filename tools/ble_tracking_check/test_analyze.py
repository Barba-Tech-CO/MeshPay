from analyze import analyze

def rec(ts, **kw):
  base = {"ts": ts, "peripheral_id": None, "service_data": None, "name": None,
          "manufacturer_data": None, "tx_power": None}
  return {**base, **kw}

def verdicts(windows):
  return {r["field"]: r for r in analyze(windows)}

def test_rotating_fields():
  w1 = [rec(0, service_data="aa", peripheral_id="P1"), rec(30, service_data="aa", peripheral_id="P1")]
  w2 = [rec(0, service_data="bb", peripheral_id="P2")]
  v = verdicts([w1, w2])
  assert v["service_data"]["verdict"].startswith("ROTATES")
  assert v["peripheral_id"]["verdict"].startswith("ROTATES")
  assert v["service_data"]["max_constant_s"] == 30
  assert v["name"]["verdict"].startswith("ABSENT")

def test_stable_field_detected():
  w1 = [rec(0, service_data="aa", name="Pixel")]
  w2 = [rec(0, service_data="bb", name="Pixel")]
  v = verdicts([w1, w2])
  assert v["name"]["verdict"].startswith("STABLE")
  assert v["service_data"]["verdict"].startswith("ROTATES")

def test_single_window_inconclusive():
  assert verdicts([[rec(0, service_data="aa")]])["service_data"]["verdict"].startswith("INCONCLUSIVE")

def test_rotation_inside_window_counts_distinct():
  w1 = [rec(0, peripheral_id="A"), rec(900, peripheral_id="B")]
  w2 = [rec(0, peripheral_id="C")]
  assert verdicts([w1, w2])["peripheral_id"]["distinct_per_window"] == [2, 1]

def test_manufacturer_dict_compared_by_value():
  m = {"76": "0102"}
  v = verdicts([[rec(0, manufacturer_data=m)], [rec(0, manufacturer_data=dict(m))]])
  assert v["manufacturer_data"]["verdict"].startswith("STABLE")
