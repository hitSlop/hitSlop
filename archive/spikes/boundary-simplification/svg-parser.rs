/// Experimental adapter. Bounds on library-internal arc expansion are evaluated separately.
struct Parser<'a> { source: &'a str }
impl<'a> Parser<'a> {
    fn new(source: &'a str) -> Result<Self> {
        if source.is_empty() || source.len() > MAX_SOURCE { return Err(invalid()); }
        let mut count = 0;
        for segment in svgtypes::PathParser::from(source) {
            let segment = segment.map_err(|_| invalid())?;
            if count == 0 && !matches!(segment, svgtypes::PathSegment::MoveTo { .. }) { return Err(invalid()); }
            count += 1;
            if count > MAX_COMMANDS { return Err(invalid()); }
        }
        if count == 0 { return Err(invalid()); }
        Ok(Self { source })
    }
    fn parse(self) -> Result<Vec<Segment>> {
        use svgtypes::SimplePathSegment as S;
        let (mut px, mut py, mut start) = (0.0, 0.0, (0.0, 0.0));
        let mut out = Vec::new();
        for segment in svgtypes::SimplifyingPathParser::from(self.source) {
            if out.len() >= 2048 { return Err(invalid()); }
            let segment = match segment.map_err(|_| invalid())? {
                S::MoveTo { x, y } => { start = (x,y); Segment::Move { x,y } },
                S::LineTo { x,y } => Segment::Line { x,y },
                S::CurveTo { x1,y1,x2,y2,x,y } => Segment::Cubic { x1,y1,x2,y2,x,y },
                S::Quadratic { x1,y1,x,y } => Segment::Cubic { x1: px + 2.0/3.0*(x1-px), y1: py + 2.0/3.0*(y1-py), x2: x + 2.0/3.0*(x1-x), y2: y + 2.0/3.0*(y1-y), x,y },
                S::ClosePath => { (px,py) = start; Segment::Close },
            };
            match segment {
                Segment::Move { x,y } | Segment::Line { x,y } => { finite(&[x,y])?; (px,py)=(x,y); },
                Segment::Cubic { x1,y1,x2,y2,x,y } => { finite(&[x1,y1,x2,y2,x,y])?; (px,py)=(x,y); },
                Segment::Close => {},
            }
            out.push(segment);
        }
        Ok(out)
    }
}
