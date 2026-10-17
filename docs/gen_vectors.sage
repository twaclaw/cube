import json
from sage.groups.perm_gps.cubegroup import CubeGroup, singmaster_indices as lab
rubik = CubeGroup()
SUF = {"": "", "'": "^-1", "2": "^2"}

# solved color of facelet p = the face it lives on = first letter of its singmaster label
solved = {p: lab[p][0] for p in range(1, 49)}

def state_after(moves):
    g = rubik.one()
    for mv in moves:
        g = g * rubik.parse(mv[0] + SUF[mv[1:]])
    dest = rubik.facets(g)          # dest[i] = image of facelet i+1
    out = [None]*48
    for i in range(48):
        out[dest[i]-1] = solved[i+1]
    return "".join(out), str(g).replace(" ", "")

cases = []
for f in "ULFRBD":
    for s in ["", "'", "2"]:
        cases.append([f+s])
cases += [["R","U"], ["R","U","R'","U'"], ["F","L","D"],
          ["U","R2","D'","F2","L2"], ["B'","D2","L","F","U'","R","B2","D"],
          ["L","L","L","L"], ["R","L","R'","L'"]]

def case(moves):
    state, perm = state_after(moves)
    return {"moves": moves, "state": state, "perm": perm}

data = {"numbering": {str(p): lab[p] for p in range(1,49)},
        "cases": [case(c) for c in cases]}
print(json.dumps(data, indent=0))
