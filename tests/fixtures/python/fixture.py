def trivial(x):
    return x

def at_limit(x):
    if x == 0: x += 1
    if x == 1: x += 1
    if x == 2: x += 1
    if x == 3: x += 1
    if x == 4: x += 1
    if x == 5: x += 1
    if x == 6: x += 1
    if x == 7: x += 1
    if x == 8: x += 1
    if x == 9: x += 1
    if x == 10: x += 1
    if x == 11: x += 1
    if x == 12: x += 1
    if x == 13: x += 1
    return x

def over_complexity(x):
    if x == 0: x += 1
    if x == 1: x += 1
    if x == 2: x += 1
    if x == 3: x += 1
    if x == 4: x += 1
    if x == 5: x += 1
    if x == 6: x += 1
    if x == 7: x += 1
    if x == 8: x += 1
    if x == 9: x += 1
    if x == 10: x += 1
    if x == 11: x += 1
    if x == 12: x += 1
    if x == 13: x += 1
    if x == 14: x += 1
    return x

def over_depth(x):
    if x:
        while x:
            for i in range(x):
                try:
                    if i: return i
                except ValueError: return 0
    return x

def over_lines(x):
    x += 0
    x += 1
    x += 2
    x += 3
    x += 4
    x += 5
    x += 6
    x += 7
    x += 8
    x += 9
    x += 10
    x += 11
    x += 12
    x += 13
    x += 14
    x += 15
    x += 16
    x += 17
    x += 18
    x += 19
    x += 20
    x += 21
    x += 22
    x += 23
    x += 24
    x += 25
    x += 26
    x += 27
    x += 28
    x += 29
    x += 30
    x += 31
    x += 32
    x += 33
    x += 34
    x += 35
    x += 36
    x += 37
    x += 38
    x += 39
    x += 40
    x += 41
    x += 42
    x += 43
    x += 44
    x += 45
    x += 46
    x += 47
    x += 48
    x += 49
    x += 50
    x += 51
    x += 52
    x += 53
    x += 54
    x += 55
    x += 56
    x += 57
    x += 58
    x += 59
    x += 60
    x += 61
    x += 62
    x += 63
    x += 64
    x += 65
    x += 66
    x += 67
    x += 68
    x += 69
    x += 70
    x += 71
    x += 72
    x += 73
    x += 74
    x += 75
    x += 76
    x += 77
    x += 78
    x += 79
    x += 80
    x += 81
    x += 82
    x += 83
    x += 84
    x += 85
    x += 86
    x += 87
    x += 88
    x += 89
    x += 90
    x += 91
    x += 92
    x += 93
    x += 94
    x += 95
    x += 96
    x += 97
    x += 98
    return x

def over_params(a, b, c, d, e, f, g):
    return a

def nested(x):
    def inner(y):
        return 1 if y else 0
    return inner(x)

def decisions(x):
    if x: x += 1
    elif x < 0: x -= 1
    for i in range(x): x += i
    while x > 10: x -= 1
    try: x += 1
    except ValueError: x -= 1
    except TypeError: x = 0
    match x:
        case 1: x += 1
        case 2: x -= 1
        case _: pass
    x = 1 if x else 0
    x = x and 1 or 0
    values = [i for i in range(x) if i and x]
    return x

class Counter:
    def increment(self, x):
        return x + 1 if x else 0
