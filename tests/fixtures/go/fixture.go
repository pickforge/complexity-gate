package fixture

func trivial(x int) int { return x }

func atLimit(x int) int {
	if x == 0 { x++ }
	if x == 1 { x++ }
	if x == 2 { x++ }
	if x == 3 { x++ }
	if x == 4 { x++ }
	if x == 5 { x++ }
	if x == 6 { x++ }
	if x == 7 { x++ }
	if x == 8 { x++ }
	if x == 9 { x++ }
	if x == 10 { x++ }
	if x == 11 { x++ }
	if x == 12 { x++ }
	if x == 13 { x++ }
	return x
}

func overComplexity(x int) int {
	if x == 0 { x++ }
	if x == 1 { x++ }
	if x == 2 { x++ }
	if x == 3 { x++ }
	if x == 4 { x++ }
	if x == 5 { x++ }
	if x == 6 { x++ }
	if x == 7 { x++ }
	if x == 8 { x++ }
	if x == 9 { x++ }
	if x == 10 { x++ }
	if x == 11 { x++ }
	if x == 12 { x++ }
	if x == 13 { x++ }
	if x == 14 { x++ }
	return x
}

func overDepth(x int) int {
	if x > 0 {
		for x > 0 {
			for i := 0; i < x; i++ {
				select {
				case <-make(chan int):
					if i > 0 { return i }
				default:
				}
			}
		}
	}
	return x
}

func overLines(x int) int {
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
}

func overParams(a,b,c,d,e,f,g int) int { return a }

func nested(x int) int {
	inner := func(y int) int { return y + 1 }
	return inner(x)
}

func decisions(x int, value any) int {
	if x > 0 { x++ }
	for i := 0; i < x; i++ { x += i }
	for _, item := range []int{x} { x += item }
	switch x { case 1: x++; case 2: x--; default: }
	switch value.(type) { case int: x++; case string: x--; default: }
	select { case <-make(chan int): x++; default: }
	if x > 0 && x < 10 || x == 20 { x++ }
	return x
}

type Counter struct{}
func (Counter) increment(x int) int { if x > 0 { return x + 1 }; return 0 }

func reviewCases(values []int, x int, err error) int {
	marker := "&& ||"
	if x == 0 { x++
	} else if x == 1 { x++
	} else if x == 2 { x++
	} else { x-- }
	if err != nil { x-- } // error-return equivalent of catch
	apply(values, func(value int) int { return value + len(marker) })
	return x
}

func commentLines(x int) int {
	/* comment-only */
	/* inline */ return x
}

func boolChain(a, b, c, d bool) bool {
	return a && (b || c) && d
}
